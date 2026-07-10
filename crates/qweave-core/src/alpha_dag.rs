use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;

use crate::alpha_eval::{
    cmp_value, correlation, covariance, decay_linear, delay, delta, group_neutralize, group_rank,
    log_value, max_value, min_value, product, quantile, rank, resi, rsquare, scale, sign,
    signed_power, slope, ts_argmax, ts_argmin, ts_max, ts_mean, ts_min, ts_rank, ts_rank_raw,
    ts_std, ts_sum, where_value,
};
use crate::cellset::CellSet;
use crate::error::{QWeaveError, Result};
use crate::expr::{CmpOp, Expr};
use crate::layout::{Layout, nt_to_tn, tn_to_nt};

/// Upper bound on nodes evaluated concurrently within one dependency level.
/// Bounds peak memory (one full-panel output per in-flight node) while staying a
/// small multiple of the core count, so every core stays busy and heavy nodes
/// still parallelize their own kernel via work-stealing.
fn max_nodes_in_flight() -> usize {
    2 * std::thread::available_parallelism().map_or(8, |n| n.get())
}

/// Cells per parallel chunk when running a fused-elementwise program.
const FUSED_EW_CHUNK: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NodeId(u32);

impl NodeId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Node {
    Field(String),
    Const(u64),
    Transpose(NodeId),
    Add(NodeId, NodeId),
    Sub(NodeId, NodeId),
    Mul(NodeId, NodeId),
    Div(NodeId, NodeId),
    Neg(NodeId),
    Delay(NodeId, usize),
    Delta(NodeId, usize),
    TsSum(NodeId, usize),
    TsMean(NodeId, usize),
    Product(NodeId, usize),
    TsMin(NodeId, usize),
    TsMax(NodeId, usize),
    TsArgMin(NodeId, usize),
    TsArgMax(NodeId, usize),
    TsRank(NodeId, usize),
    TsRankRaw(NodeId, usize),
    TsStd(NodeId, usize),
    Slope(NodeId, usize),
    Rsquare(NodeId, usize),
    Resi(NodeId, usize),
    Quantile(NodeId, usize, u64),
    DecayLinear(NodeId, usize),
    Correlation(NodeId, NodeId, usize),
    Covariance(NodeId, NodeId, usize),
    Rank(NodeId),
    Scale(NodeId, u64),
    GroupRank(NodeId, NodeId),
    GroupNeutralize(NodeId, NodeId),
    Abs(NodeId),
    Log(NodeId),
    Sign(NodeId),
    SignedPower(NodeId, NodeId),
    Power(NodeId, NodeId),
    Min(NodeId, NodeId),
    Max(NodeId, NodeId),
    Cmp(CmpOp, NodeId, NodeId),
    Where(NodeId, NodeId, NodeId),
    /// A maximal single-use elementwise subtree collapsed into one node: `leaves`
    /// are the (materialized) cell inputs and `program` is the RPN evaluated once
    /// per cell, so the chain runs in a single pass with no intermediate buffers.
    FusedEw {
        leaves: Vec<NodeId>,
        program: Vec<EwOp>,
    },
}

/// One step of a fused-elementwise RPN program, evaluated against a small value
/// stack. `Leaf(i)` pushes cell `i` of `leaves[i]`; the rest mirror the
/// elementwise `Node` operators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EwOp {
    Leaf(u32),
    Const(u64),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Abs,
    Log,
    Sign,
    SignedPower,
    Power,
    Min,
    Max,
    Cmp(CmpOp),
    Where,
}

impl Node {
    fn visit_children(&self, mut visit: impl FnMut(NodeId)) {
        match self {
            Node::Field(_) | Node::Const(_) => {}
            Node::Transpose(inner)
            | Node::Neg(inner)
            | Node::Delay(inner, _)
            | Node::Delta(inner, _)
            | Node::TsSum(inner, _)
            | Node::TsMean(inner, _)
            | Node::Product(inner, _)
            | Node::TsMin(inner, _)
            | Node::TsMax(inner, _)
            | Node::TsArgMin(inner, _)
            | Node::TsArgMax(inner, _)
            | Node::TsRank(inner, _)
            | Node::TsRankRaw(inner, _)
            | Node::TsStd(inner, _)
            | Node::Slope(inner, _)
            | Node::Rsquare(inner, _)
            | Node::Resi(inner, _)
            | Node::Quantile(inner, _, _)
            | Node::DecayLinear(inner, _)
            | Node::Rank(inner)
            | Node::Scale(inner, _)
            | Node::Abs(inner)
            | Node::Log(inner)
            | Node::Sign(inner) => visit(*inner),
            Node::Add(lhs, rhs)
            | Node::Sub(lhs, rhs)
            | Node::Mul(lhs, rhs)
            | Node::Div(lhs, rhs)
            | Node::Correlation(lhs, rhs, _)
            | Node::Covariance(lhs, rhs, _)
            | Node::GroupRank(lhs, rhs)
            | Node::GroupNeutralize(lhs, rhs)
            | Node::SignedPower(lhs, rhs)
            | Node::Power(lhs, rhs)
            | Node::Min(lhs, rhs)
            | Node::Max(lhs, rhs)
            | Node::Cmp(_, lhs, rhs) => {
                visit(*lhs);
                visit(*rhs);
            }
            Node::Where(cond, when_true, when_false) => {
                visit(*cond);
                visit(*when_true);
                visit(*when_false);
            }
            Node::FusedEw { leaves, .. } => {
                for leaf in leaves {
                    visit(*leaf);
                }
            }
        }
    }

    /// Rebuild this node with every child id passed through `map` (used to
    /// rewrite references when a transpose is bypassed).
    fn map_children(&self, mut map: impl FnMut(NodeId) -> NodeId) -> Node {
        match self {
            Node::Field(name) => Node::Field(name.clone()),
            Node::Const(bits) => Node::Const(*bits),
            Node::Transpose(inner) => Node::Transpose(map(*inner)),
            Node::Add(lhs, rhs) => Node::Add(map(*lhs), map(*rhs)),
            Node::Sub(lhs, rhs) => Node::Sub(map(*lhs), map(*rhs)),
            Node::Mul(lhs, rhs) => Node::Mul(map(*lhs), map(*rhs)),
            Node::Div(lhs, rhs) => Node::Div(map(*lhs), map(*rhs)),
            Node::Neg(inner) => Node::Neg(map(*inner)),
            Node::Delay(inner, days) => Node::Delay(map(*inner), *days),
            Node::Delta(inner, days) => Node::Delta(map(*inner), *days),
            Node::TsSum(inner, days) => Node::TsSum(map(*inner), *days),
            Node::TsMean(inner, days) => Node::TsMean(map(*inner), *days),
            Node::Product(inner, days) => Node::Product(map(*inner), *days),
            Node::TsMin(inner, days) => Node::TsMin(map(*inner), *days),
            Node::TsMax(inner, days) => Node::TsMax(map(*inner), *days),
            Node::TsArgMin(inner, days) => Node::TsArgMin(map(*inner), *days),
            Node::TsArgMax(inner, days) => Node::TsArgMax(map(*inner), *days),
            Node::TsRank(inner, days) => Node::TsRank(map(*inner), *days),
            Node::TsRankRaw(inner, days) => Node::TsRankRaw(map(*inner), *days),
            Node::TsStd(inner, days) => Node::TsStd(map(*inner), *days),
            Node::Slope(inner, days) => Node::Slope(map(*inner), *days),
            Node::Rsquare(inner, days) => Node::Rsquare(map(*inner), *days),
            Node::Resi(inner, days) => Node::Resi(map(*inner), *days),
            Node::Quantile(inner, days, q) => Node::Quantile(map(*inner), *days, *q),
            Node::DecayLinear(inner, days) => Node::DecayLinear(map(*inner), *days),
            Node::Correlation(lhs, rhs, days) => Node::Correlation(map(*lhs), map(*rhs), *days),
            Node::Covariance(lhs, rhs, days) => Node::Covariance(map(*lhs), map(*rhs), *days),
            Node::Rank(inner) => Node::Rank(map(*inner)),
            Node::Scale(inner, scale_to) => Node::Scale(map(*inner), *scale_to),
            Node::GroupRank(values, groups) => Node::GroupRank(map(*values), map(*groups)),
            Node::GroupNeutralize(values, groups) => {
                Node::GroupNeutralize(map(*values), map(*groups))
            }
            Node::Abs(inner) => Node::Abs(map(*inner)),
            Node::Log(inner) => Node::Log(map(*inner)),
            Node::Sign(inner) => Node::Sign(map(*inner)),
            Node::SignedPower(lhs, rhs) => Node::SignedPower(map(*lhs), map(*rhs)),
            Node::Power(lhs, rhs) => Node::Power(map(*lhs), map(*rhs)),
            Node::Min(lhs, rhs) => Node::Min(map(*lhs), map(*rhs)),
            Node::Max(lhs, rhs) => Node::Max(map(*lhs), map(*rhs)),
            Node::Cmp(op, lhs, rhs) => Node::Cmp(*op, map(*lhs), map(*rhs)),
            Node::Where(cond, when_true, when_false) => {
                Node::Where(map(*cond), map(*when_true), map(*when_false))
            }
            Node::FusedEw { leaves, program } => Node::FusedEw {
                leaves: leaves.iter().map(|leaf| map(*leaf)).collect(),
                program: program.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueLayout {
    Scalar,
    Cells(Layout),
}

#[derive(Default)]
struct Dag {
    nodes: Vec<Node>,
    layouts: Vec<ValueLayout>,
    by_node: HashMap<Node, NodeId>,
}

impl Dag {
    #[cfg(test)]
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn lower(&mut self, expr: &Expr) -> NodeId {
        match expr {
            Expr::Field(name) => {
                self.intern(Node::Field(name.clone()), ValueLayout::Cells(Layout::Nt))
            }
            Expr::Const(value) => self.constant(*value),
            Expr::Add(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::Add, |lhs, rhs| lhs + rhs)
            }
            Expr::Sub(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::Sub, |lhs, rhs| lhs - rhs)
            }
            Expr::Mul(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::Mul, |lhs, rhs| lhs * rhs)
            }
            Expr::Div(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::Div, |lhs, rhs| lhs / rhs)
            }
            Expr::Neg(inner) => self.lower_unary_elementwise(inner, Node::Neg, |value| -value),
            Expr::Delay(inner, days) => self.lower_ts_unary(inner, *days, Node::Delay),
            Expr::Delta(inner, days) => self.lower_ts_unary(inner, *days, Node::Delta),
            Expr::TsSum(inner, days) => self.lower_ts_unary(inner, *days, Node::TsSum),
            Expr::TsMean(inner, days) => self.lower_ts_unary(inner, *days, Node::TsMean),
            Expr::Product(inner, days) => self.lower_ts_unary(inner, *days, Node::Product),
            Expr::TsMin(inner, days) => self.lower_ts_unary(inner, *days, Node::TsMin),
            Expr::TsMax(inner, days) => self.lower_ts_unary(inner, *days, Node::TsMax),
            Expr::TsArgMin(inner, days) => self.lower_ts_unary(inner, *days, Node::TsArgMin),
            Expr::TsArgMax(inner, days) => self.lower_ts_unary(inner, *days, Node::TsArgMax),
            Expr::TsRank(inner, days) => self.lower_ts_unary(inner, *days, Node::TsRank),
            Expr::TsRankRaw(inner, days) => self.lower_ts_unary(inner, *days, Node::TsRankRaw),
            Expr::TsStd(inner, days) => self.lower_ts_unary(inner, *days, Node::TsStd),
            Expr::Slope(inner, days) => self.lower_ts_unary(inner, *days, Node::Slope),
            Expr::Rsquare(inner, days) => self.lower_ts_unary(inner, *days, Node::Rsquare),
            Expr::Resi(inner, days) => self.lower_ts_unary(inner, *days, Node::Resi),
            Expr::Quantile(inner, days, q) => {
                let inner = self.lower_to(inner, Layout::Nt);
                self.intern(
                    Node::Quantile(inner, *days, q.to_bits()),
                    ValueLayout::Cells(Layout::Nt),
                )
            }
            Expr::DecayLinear(inner, days) => self.lower_ts_unary(inner, *days, Node::DecayLinear),
            Expr::Correlation(lhs, rhs, days) => {
                let lhs = self.lower_to(lhs, Layout::Nt);
                let rhs = self.lower_to(rhs, Layout::Nt);
                self.intern(
                    Node::Correlation(lhs, rhs, *days),
                    ValueLayout::Cells(Layout::Nt),
                )
            }
            Expr::Covariance(lhs, rhs, days) => {
                let lhs = self.lower_to(lhs, Layout::Nt);
                let rhs = self.lower_to(rhs, Layout::Nt);
                self.intern(
                    Node::Covariance(lhs, rhs, *days),
                    ValueLayout::Cells(Layout::Nt),
                )
            }
            Expr::Rank(inner) => {
                let inner = self.lower_to(inner, Layout::Tn);
                self.intern(Node::Rank(inner), ValueLayout::Cells(Layout::Tn))
            }
            Expr::Scale(inner, scale_to) => {
                let inner = self.lower_to(inner, Layout::Tn);
                self.intern(
                    Node::Scale(inner, scale_to.to_bits()),
                    ValueLayout::Cells(Layout::Tn),
                )
            }
            Expr::GroupRank(values, groups) => {
                let values = self.lower_to(values, Layout::Tn);
                let groups = self.lower_to(groups, Layout::Tn);
                self.intern(
                    Node::GroupRank(values, groups),
                    ValueLayout::Cells(Layout::Tn),
                )
            }
            Expr::GroupNeutralize(values, groups) => {
                let values = self.lower_to(values, Layout::Tn);
                let groups = self.lower_to(groups, Layout::Tn);
                self.intern(
                    Node::GroupNeutralize(values, groups),
                    ValueLayout::Cells(Layout::Tn),
                )
            }
            Expr::Abs(inner) => self.lower_unary_elementwise(inner, Node::Abs, f64::abs),
            Expr::Log(inner) => self.lower_unary_elementwise(inner, Node::Log, log_value),
            Expr::Sign(inner) => self.lower_unary_elementwise(inner, Node::Sign, sign),
            Expr::SignedPower(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::SignedPower, signed_power)
            }
            Expr::Power(lhs, rhs) => {
                self.lower_binary_elementwise(lhs, rhs, Node::Power, |value, exponent| {
                    value.powf(exponent)
                })
            }
            Expr::Min(lhs, rhs) => self.lower_binary_elementwise(lhs, rhs, Node::Min, min_value),
            Expr::Max(lhs, rhs) => self.lower_binary_elementwise(lhs, rhs, Node::Max, max_value),
            Expr::Cmp(op, lhs, rhs) => {
                let op = *op;
                self.lower_binary_elementwise(lhs, rhs, |lhs, rhs| Node::Cmp(op, lhs, rhs), {
                    move |lhs, rhs| cmp_value(op, lhs, rhs)
                })
            }
            Expr::Where(cond, when_true, when_false) => {
                let cond = self.lower(cond);
                let when_true = self.lower(when_true);
                let when_false = self.lower(when_false);

                if let (Some(cond), Some(when_true), Some(when_false)) = (
                    self.const_value(cond),
                    self.const_value(when_true),
                    self.const_value(when_false),
                ) {
                    return self.constant(where_value(cond, when_true, when_false));
                }

                let layout = self.first_cells_layout(&[cond, when_true, when_false]);
                let (cond, when_true, when_false, output_layout) = match layout {
                    Some(layout) => (
                        self.coerce_layout(cond, layout),
                        self.coerce_layout(when_true, layout),
                        self.coerce_layout(when_false, layout),
                        ValueLayout::Cells(layout),
                    ),
                    None => (cond, when_true, when_false, ValueLayout::Scalar),
                };
                self.intern(Node::Where(cond, when_true, when_false), output_layout)
            }
        }
    }

    fn lower_to(&mut self, expr: &Expr, want: Layout) -> NodeId {
        let id = self.lower(expr);
        self.coerce_layout(id, want)
    }

    fn lower_ts_unary(
        &mut self,
        inner: &Expr,
        days: usize,
        node: impl FnOnce(NodeId, usize) -> Node,
    ) -> NodeId {
        let inner = self.lower_to(inner, Layout::Nt);
        self.intern(node(inner, days), ValueLayout::Cells(Layout::Nt))
    }

    fn lower_unary_elementwise(
        &mut self,
        inner: &Expr,
        node: impl FnOnce(NodeId) -> Node,
        op: impl FnOnce(f64) -> f64,
    ) -> NodeId {
        let inner = self.lower(inner);
        if let Some(value) = self.const_value(inner) {
            return self.constant(op(value));
        }
        self.intern(node(inner), self.layout(inner))
    }

    fn lower_binary_elementwise(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        node: impl FnOnce(NodeId, NodeId) -> Node,
        op: impl FnOnce(f64, f64) -> f64,
    ) -> NodeId {
        let lhs = self.lower(lhs);
        let rhs = self.lower(rhs);

        if let (Some(lhs_value), Some(rhs_value)) = (self.const_value(lhs), self.const_value(rhs)) {
            return self.constant(op(lhs_value, rhs_value));
        }

        let layout = self.first_cells_layout(&[lhs, rhs]);
        let (lhs, rhs, output_layout) = match layout {
            Some(layout) => (
                self.coerce_layout(lhs, layout),
                self.coerce_layout(rhs, layout),
                ValueLayout::Cells(layout),
            ),
            None => (lhs, rhs, ValueLayout::Scalar),
        };
        self.intern(node(lhs, rhs), output_layout)
    }

    fn coerce_layout(&mut self, id: NodeId, want: Layout) -> NodeId {
        match self.layout(id) {
            ValueLayout::Scalar => id,
            ValueLayout::Cells(layout) if layout == want => id,
            ValueLayout::Cells(_) => self.intern(Node::Transpose(id), ValueLayout::Cells(want)),
        }
    }

    fn first_cells_layout(&self, ids: &[NodeId]) -> Option<Layout> {
        ids.iter().find_map(|id| match self.layout(*id) {
            ValueLayout::Scalar => None,
            ValueLayout::Cells(layout) => Some(layout),
        })
    }

    fn const_value(&self, id: NodeId) -> Option<f64> {
        match self.nodes[id.index()] {
            Node::Const(bits) => Some(f64::from_bits(bits)),
            _ => None,
        }
    }

    fn constant(&mut self, value: f64) -> NodeId {
        self.intern(Node::Const(value.to_bits()), ValueLayout::Scalar)
    }

    fn intern(&mut self, node: Node, layout: ValueLayout) -> NodeId {
        if let Some(id) = self.by_node.get(&node).copied() {
            debug_assert_eq!(self.layout(id), layout);
            return id;
        }

        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node.clone());
        self.layouts.push(layout);
        self.by_node.insert(node, id);
        id
    }

    fn layout(&self, id: NodeId) -> ValueLayout {
        self.layouts[id.index()]
    }

    /// Output layout of a cross-sectional node (always cells, never scalar).
    fn cells_layout(&self, id: NodeId) -> Layout {
        match self.layout(id) {
            ValueLayout::Cells(layout) => layout,
            ValueLayout::Scalar => Layout::Tn,
        }
    }

    fn eval_roots(&self, roots: &[NodeId], cs: &CellSet) -> Result<Vec<Arc<DagVal>>> {
        let order = self.reachable_order(roots);
        let levels = self.level_buckets(&order);
        let mut remaining_consumers = self.consumer_counts(&order, roots);
        let mut slots: Vec<Option<Arc<DagVal>>> = vec![None; self.nodes.len()];
        let max_in_flight = max_nodes_in_flight();

        for level in &levels {
            // Every child of a node sits in a strictly lower level, so nodes in
            // one level are mutually independent. We still cap how many run at
            // once: a wide level holds one full-panel output per node, so without
            // a bound the peak is the widest level. Evaluate the level in chunks
            // that keep every core busy (heavy nodes still parallelize their own
            // kernel) while only `MAX_NODES_IN_FLIGHT` outputs are live at a time.
            for chunk in level.chunks(max_in_flight) {
                // The parallel phase only reads the already-filled slots, so the
                // borrows can't overlap and there is no data race; results are
                // installed sequentially afterwards.
                let computed = chunk
                    .par_iter()
                    .map(|&id| {
                        self.eval_node(id, &slots, cs)
                            .map(|value| (id, Arc::new(value)))
                    })
                    .collect::<Result<Vec<_>>>()?;
                for (id, value) in computed {
                    slots[id.index()] = Some(value);
                }
                // Release any child whose last consumer has now run, capping peak
                // memory the same way the sequential evaluator did.
                for &id in chunk {
                    self.nodes[id.index()].visit_children(|child| {
                        let remaining = &mut remaining_consumers[child.index()];
                        *remaining = remaining
                            .checked_sub(1)
                            .expect("child consumer count underflow");
                        if *remaining == 0 {
                            slots[child.index()] = None;
                        }
                    });
                }
            }
        }

        let values = roots
            .iter()
            .map(|root| {
                Arc::clone(
                    slots[root.index()]
                        .as_ref()
                        .expect("root slot should still be retained"),
                )
            })
            .collect();

        Ok(values)
    }

    /// Group reachable nodes by dependency depth. `order` is ascending index =
    /// topological order, so each child's level is final before its parent.
    fn level_buckets(&self, order: &[NodeId]) -> Vec<Vec<NodeId>> {
        let mut level = vec![0usize; self.nodes.len()];
        let mut max_level = 0;
        for &id in order {
            let mut node_level = 0;
            self.nodes[id.index()].visit_children(|child| {
                node_level = node_level.max(level[child.index()] + 1);
            });
            level[id.index()] = node_level;
            max_level = max_level.max(node_level);
        }
        let mut buckets = vec![Vec::new(); max_level + 1];
        for &id in order {
            buckets[level[id.index()]].push(id);
        }
        buckets
    }

    fn reachable_order(&self, roots: &[NodeId]) -> Vec<NodeId> {
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack = roots.to_vec();
        while let Some(id) = stack.pop() {
            if std::mem::replace(&mut reachable[id.index()], true) {
                continue;
            }
            self.nodes[id.index()].visit_children(|child| stack.push(child));
        }

        reachable
            .into_iter()
            .enumerate()
            .filter_map(|(idx, is_reachable)| is_reachable.then_some(NodeId(idx as u32)))
            .collect()
    }

    fn consumer_counts(&self, order: &[NodeId], roots: &[NodeId]) -> Vec<usize> {
        let mut counts = vec![0; self.nodes.len()];
        for root in roots {
            counts[root.index()] += 1;
        }
        for id in order {
            self.nodes[id.index()].visit_children(|child| counts[child.index()] += 1);
        }
        counts
    }

    /// Drop the materialized transpose in front of a cross-sectional op when that
    /// op is its only consumer: the op gathers its input directly from the source
    /// layout instead (see `xs_per_block`/`xs_per_group`). A transpose with two or
    /// more consumers stays — materializing once and sharing the contiguous Tn
    /// buffer beats re-gathering it per consumer. Value-preserving (the gather
    /// reads the same cells), so the golden baseline is unaffected.
    fn fuse_single_use_transposes(&mut self, roots: &[NodeId]) {
        let order = self.reachable_order(roots);
        let counts = self.consumer_counts(&order, roots);
        for &id in &order {
            let fused = match &self.nodes[id.index()] {
                Node::Rank(inner) => Node::Rank(self.bypass_lone_transpose(*inner, &counts)),
                Node::Scale(inner, scale_to) => {
                    Node::Scale(self.bypass_lone_transpose(*inner, &counts), *scale_to)
                }
                Node::GroupRank(values, groups) => Node::GroupRank(
                    self.bypass_lone_transpose(*values, &counts),
                    self.bypass_lone_transpose(*groups, &counts),
                ),
                Node::GroupNeutralize(values, groups) => Node::GroupNeutralize(
                    self.bypass_lone_transpose(*values, &counts),
                    self.bypass_lone_transpose(*groups, &counts),
                ),
                _ => continue,
            };
            self.nodes[id.index()] = fused;
        }
    }

    fn bypass_lone_transpose(&self, input: NodeId, counts: &[usize]) -> NodeId {
        if counts[input.index()] == 1 {
            if let Node::Transpose(child) = self.nodes[input.index()] {
                return child;
            }
        }
        input
    }

    /// Mirror of the input fusion on the output side: when a cross-sectional op's
    /// only consumer is a transpose to Nt (e.g. it feeds a time-series op), flip
    /// the op to emit Nt directly (it scatters its result into Nt) and drop the
    /// transpose. A shared output (the op also consumed as Tn) keeps materializing
    /// once. Value-preserving — same cells, different storage order.
    fn fuse_xs_output_transposes(&mut self, roots: &[NodeId]) {
        let order = self.reachable_order(roots);
        let counts = self.consumer_counts(&order, roots);
        let mut remap: HashMap<NodeId, NodeId> = HashMap::new();
        for &id in &order {
            let child = match &self.nodes[id.index()] {
                Node::Transpose(child) => *child,
                _ => continue,
            };
            if counts[child.index()] == 1
                && self.layout(id) == ValueLayout::Cells(Layout::Nt)
                && self.is_xs_op(child)
            {
                self.layouts[child.index()] = ValueLayout::Cells(Layout::Nt);
                remap.insert(id, child);
            }
        }
        if remap.is_empty() {
            return;
        }
        for &id in &order {
            let mapped = self.nodes[id.index()]
                .map_children(|child| remap.get(&child).copied().unwrap_or(child));
            self.nodes[id.index()] = mapped;
        }
    }

    fn is_xs_op(&self, id: NodeId) -> bool {
        matches!(
            self.nodes[id.index()],
            Node::Rank(_) | Node::Scale(..) | Node::GroupRank(..) | Node::GroupNeutralize(..)
        )
    }

    /// Collapse each maximal single-use elementwise subtree into one `FusedEw`
    /// node so the whole chain runs in a single per-cell pass (no intermediate
    /// buffers). Value-preserving: the RPN performs exactly the same operations
    /// in the same order, so the golden baseline is unaffected.
    fn fuse_elementwise(&mut self, roots: &[NodeId]) {
        let order = self.reachable_order(roots);
        let counts = self.consumer_counts(&order, roots);
        // An elementwise node is absorbed into its (single, elementwise) parent
        // rather than materialized; anything else feeding an elementwise node is a
        // leaf of the fused subtree.
        let mut absorbed = vec![false; self.nodes.len()];
        for &id in &order {
            if is_elementwise(&self.nodes[id.index()]) {
                self.nodes[id.index()].visit_children(|child| {
                    if counts[child.index()] == 1 && is_elementwise(&self.nodes[child.index()]) {
                        absorbed[child.index()] = true;
                    }
                });
            }
        }
        for &id in &order {
            if is_elementwise(&self.nodes[id.index()]) && !absorbed[id.index()] {
                let mut leaves = Vec::new();
                let mut leaf_index = HashMap::new();
                let mut program = Vec::new();
                self.emit_ew(id, &absorbed, &mut leaves, &mut leaf_index, &mut program);
                // Only worth a fused node when at least two operators were merged;
                // a lone op keeps its vectorized kernel (the VM would be slower).
                let operators = program
                    .iter()
                    .filter(|op| !matches!(op, EwOp::Leaf(_) | EwOp::Const(_)))
                    .count();
                if operators >= 2 {
                    self.nodes[id.index()] = Node::FusedEw { leaves, program };
                }
            }
        }
    }

    fn emit_ew(
        &self,
        id: NodeId,
        absorbed: &[bool],
        leaves: &mut Vec<NodeId>,
        leaf_index: &mut HashMap<NodeId, u32>,
        program: &mut Vec<EwOp>,
    ) {
        macro_rules! unary {
            ($a:expr, $op:expr) => {{
                self.emit_ew_child(*$a, absorbed, leaves, leaf_index, program);
                program.push($op);
            }};
        }
        macro_rules! binary {
            ($a:expr, $b:expr, $op:expr) => {{
                self.emit_ew_child(*$a, absorbed, leaves, leaf_index, program);
                self.emit_ew_child(*$b, absorbed, leaves, leaf_index, program);
                program.push($op);
            }};
        }
        match &self.nodes[id.index()] {
            Node::Add(a, b) => binary!(a, b, EwOp::Add),
            Node::Sub(a, b) => binary!(a, b, EwOp::Sub),
            Node::Mul(a, b) => binary!(a, b, EwOp::Mul),
            Node::Div(a, b) => binary!(a, b, EwOp::Div),
            Node::Neg(a) => unary!(a, EwOp::Neg),
            Node::Abs(a) => unary!(a, EwOp::Abs),
            Node::Log(a) => unary!(a, EwOp::Log),
            Node::Sign(a) => unary!(a, EwOp::Sign),
            Node::SignedPower(a, b) => binary!(a, b, EwOp::SignedPower),
            Node::Power(a, b) => binary!(a, b, EwOp::Power),
            Node::Min(a, b) => binary!(a, b, EwOp::Min),
            Node::Max(a, b) => binary!(a, b, EwOp::Max),
            Node::Cmp(op, a, b) => binary!(a, b, EwOp::Cmp(*op)),
            Node::Where(cond, when_true, when_false) => {
                self.emit_ew_child(*cond, absorbed, leaves, leaf_index, program);
                self.emit_ew_child(*when_true, absorbed, leaves, leaf_index, program);
                self.emit_ew_child(*when_false, absorbed, leaves, leaf_index, program);
                program.push(EwOp::Where);
            }
            _ => unreachable!("emit_ew called on non-elementwise node"),
        }
    }

    fn emit_ew_child(
        &self,
        child: NodeId,
        absorbed: &[bool],
        leaves: &mut Vec<NodeId>,
        leaf_index: &mut HashMap<NodeId, u32>,
        program: &mut Vec<EwOp>,
    ) {
        if absorbed[child.index()] {
            self.emit_ew(child, absorbed, leaves, leaf_index, program);
        } else if let Node::Const(bits) = self.nodes[child.index()] {
            program.push(EwOp::Const(bits));
        } else {
            let next = leaves.len() as u32;
            let index = *leaf_index.entry(child).or_insert(next);
            if index == next {
                leaves.push(child);
            }
            program.push(EwOp::Leaf(index));
        }
    }

    fn eval_node(&self, id: NodeId, slots: &[Option<Arc<DagVal>>], cs: &CellSet) -> Result<DagVal> {
        Ok(match self.nodes[id.index()] {
            Node::Field(ref name) => {
                let values = cs
                    .fields
                    .get(name)
                    .ok_or_else(|| QWeaveError::MissingColumn(name.clone()))?;
                DagVal::Cells {
                    values: Arc::clone(values),
                    layout: Layout::Nt,
                }
            }
            Node::Const(bits) => DagVal::Scalar(f64::from_bits(bits)),
            Node::Transpose(inner) => match slot_value(slots, inner) {
                DagVal::Scalar(value) => DagVal::Scalar(*value),
                DagVal::Cells {
                    values,
                    layout: Layout::Nt,
                } => DagVal::Cells {
                    values: Arc::new(nt_to_tn(values, cs)),
                    layout: Layout::Tn,
                },
                DagVal::Cells {
                    values,
                    layout: Layout::Tn,
                } => DagVal::Cells {
                    values: Arc::new(tn_to_nt(values, cs)),
                    layout: Layout::Nt,
                },
            },
            Node::Add(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                |a, b| a + b,
            ),
            Node::Sub(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                |a, b| a - b,
            ),
            Node::Mul(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                |a, b| a * b,
            ),
            Node::Div(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                |a, b| a / b,
            ),
            Node::Neg(inner) => eval_unary_elementwise(slot_value(slots, inner), |value| -value),
            Node::Delay(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| delay(values, days, cs),
            ),
            Node::Delta(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| delta(values, days, cs),
            ),
            Node::TsSum(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_sum(values, days, cs),
            ),
            Node::TsMean(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_mean(values, days, cs),
            ),
            Node::Product(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| product(values, days, cs),
            ),
            Node::TsMin(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_min(values, days, cs),
            ),
            Node::TsMax(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_max(values, days, cs),
            ),
            Node::TsArgMin(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_argmin(values, days, cs),
            ),
            Node::TsArgMax(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_argmax(values, days, cs),
            ),
            Node::TsRank(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_rank(values, days, cs),
            ),
            Node::TsRankRaw(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_rank_raw(values, days, cs),
            ),
            Node::TsStd(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_std(values, days, cs),
            ),
            Node::Slope(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| slope(values, days, cs),
            ),
            Node::Rsquare(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| rsquare(values, days, cs),
            ),
            Node::Resi(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| resi(values, days, cs),
            ),
            Node::Quantile(inner, days, q) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| quantile(values, days, f64::from_bits(q), cs),
            ),
            Node::DecayLinear(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| decay_linear(values, days, cs),
            ),
            Node::Correlation(lhs, rhs, days) => eval_cells_binary(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                Layout::Nt,
                Layout::Nt,
                cs,
                |lhs, rhs, cs| correlation(lhs, rhs, days, cs),
            ),
            Node::Covariance(lhs, rhs, days) => eval_cells_binary(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                Layout::Nt,
                Layout::Nt,
                cs,
                |lhs, rhs, cs| covariance(lhs, rhs, days, cs),
            ),
            Node::Rank(inner) => {
                eval_xs_unary(slot_value(slots, inner), self.cells_layout(id), cs, rank)
            }
            Node::Scale(inner, scale_to) => eval_xs_unary(
                slot_value(slots, inner),
                self.cells_layout(id),
                cs,
                |values, input_layout, output, cs| {
                    scale(values, input_layout, output, f64::from_bits(scale_to), cs)
                },
            ),
            Node::GroupRank(values, groups) => eval_xs_binary(
                slot_value(slots, values),
                slot_value(slots, groups),
                self.cells_layout(id),
                cs,
                group_rank,
            ),
            Node::GroupNeutralize(values, groups) => eval_xs_binary(
                slot_value(slots, values),
                slot_value(slots, groups),
                self.cells_layout(id),
                cs,
                group_neutralize,
            ),
            Node::Abs(inner) => eval_unary_elementwise(slot_value(slots, inner), f64::abs),
            Node::Log(inner) => eval_unary_elementwise(slot_value(slots, inner), log_value),
            Node::Sign(inner) => eval_unary_elementwise(slot_value(slots, inner), sign),
            Node::SignedPower(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                signed_power,
            ),
            Node::Power(lhs, rhs) => {
                eval_binary_elementwise(slot_value(slots, lhs), slot_value(slots, rhs), cs, {
                    |value, exponent| value.powf(exponent)
                })
            }
            Node::Min(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                min_value,
            ),
            Node::Max(lhs, rhs) => eval_binary_elementwise(
                slot_value(slots, lhs),
                slot_value(slots, rhs),
                cs,
                max_value,
            ),
            Node::Cmp(op, lhs, rhs) => {
                eval_binary_elementwise(slot_value(slots, lhs), slot_value(slots, rhs), cs, {
                    |lhs, rhs| cmp_value(op, lhs, rhs)
                })
            }
            Node::Where(cond, when_true, when_false) => eval_where(
                slot_value(slots, cond),
                slot_value(slots, when_true),
                slot_value(slots, when_false),
                cs,
            ),
            Node::FusedEw {
                ref leaves,
                ref program,
            } => eval_fused_ew(leaves, program, self.cells_layout(id), slots, cs),
        })
    }
}

fn is_elementwise(node: &Node) -> bool {
    matches!(
        node,
        Node::Add(..)
            | Node::Sub(..)
            | Node::Mul(..)
            | Node::Div(..)
            | Node::Neg(..)
            | Node::Abs(..)
            | Node::Log(..)
            | Node::Sign(..)
            | Node::SignedPower(..)
            | Node::Power(..)
            | Node::Min(..)
            | Node::Max(..)
            | Node::Cmp(..)
            | Node::Where(..)
    )
}

/// Evaluate a fused elementwise node: read each leaf in the shared layout, then
/// run the RPN once per cell over a small reused stack. Parallel across cell
/// chunks so a lone heavy fused node still uses every core (like the kernels).
/// Peak value-stack depth an RPN `program` reaches — the number of chunk-wide
/// registers the columnar interpreter must hold. The peak is always hit right
/// after a push, so recording the running depth after every op suffices.
fn program_max_depth(program: &[EwOp]) -> usize {
    let mut depth = 0usize;
    let mut max = 0usize;
    for op in program {
        match op {
            EwOp::Leaf(_) | EwOp::Const(_) => depth += 1,
            EwOp::Neg | EwOp::Abs | EwOp::Log | EwOp::Sign => {}
            EwOp::Add
            | EwOp::Sub
            | EwOp::Mul
            | EwOp::Div
            | EwOp::SignedPower
            | EwOp::Power
            | EwOp::Min
            | EwOp::Max
            | EwOp::Cmp(_) => depth -= 1,
            EwOp::Where => depth -= 2,
        }
        max = max.max(depth);
    }
    max
}

fn eval_fused_ew(
    leaves: &[NodeId],
    program: &[EwOp],
    layout: Layout,
    slots: &[Option<Arc<DagVal>>],
    cs: &CellSet,
) -> DagVal {
    let leaf_cells: Vec<Cow<[f64]>> = leaves
        .iter()
        .map(|leaf| cells_for(slot_value(slots, *leaf), layout, cs))
        .collect();
    let max_depth = program_max_depth(program);
    let mut out = vec![0.0f64; cs.n_cells];
    // Columnar interpreter: rather than replaying the whole program per cell, run
    // one op across a full chunk at a time so the inner cell loops auto-vectorize.
    // `regs` is a stack of `max_depth` chunk-wide registers laid out end to end;
    // register `r` occupies `regs[r*W .. (r+1)*W]`. Each lane sees the same op
    // sequence on the same operands as the scalar version, so it is bit-exact.
    const W: usize = FUSED_EW_CHUNK;
    out.par_chunks_mut(W)
        .enumerate()
        .for_each(|(chunk, out_chunk)| {
            let base = chunk * W;
            let len = out_chunk.len();
            let mut regs = vec![0.0f64; max_depth * W];
            let mut sp = 0usize;

            macro_rules! unary {
                ($f:expr) => {{
                    for x in &mut regs[(sp - 1) * W..(sp - 1) * W + len] {
                        *x = $f(*x);
                    }
                }};
            }
            macro_rules! binary {
                ($f:expr) => {{
                    sp -= 1;
                    let (left, right) = regs.split_at_mut(sp * W);
                    let a = &mut left[(sp - 1) * W..(sp - 1) * W + len];
                    let b = &right[..len];
                    for i in 0..len {
                        a[i] = $f(a[i], b[i]);
                    }
                }};
            }

            for op in program {
                match op {
                    EwOp::Leaf(j) => {
                        regs[sp * W..sp * W + len]
                            .copy_from_slice(&leaf_cells[*j as usize][base..base + len]);
                        sp += 1;
                    }
                    EwOp::Const(bits) => {
                        regs[sp * W..sp * W + len].fill(f64::from_bits(*bits));
                        sp += 1;
                    }
                    EwOp::Add => binary!(|a, b| a + b),
                    EwOp::Sub => binary!(|a, b| a - b),
                    EwOp::Mul => binary!(|a, b| a * b),
                    EwOp::Div => binary!(|a, b| a / b),
                    EwOp::Neg => unary!(|a: f64| -a),
                    EwOp::Abs => unary!(f64::abs),
                    EwOp::Log => unary!(log_value),
                    EwOp::Sign => unary!(sign),
                    EwOp::SignedPower => binary!(signed_power),
                    EwOp::Power => binary!(|a: f64, b| a.powf(b)),
                    EwOp::Min => binary!(min_value),
                    EwOp::Max => binary!(max_value),
                    EwOp::Cmp(cmp) => {
                        let cmp = *cmp;
                        binary!(move |a, b| cmp_value(cmp, a, b));
                    }
                    EwOp::Where => {
                        sp -= 2;
                        let (left, right) = regs.split_at_mut(sp * W);
                        let cond = &mut left[(sp - 1) * W..(sp - 1) * W + len];
                        let when_true = &right[..len];
                        let when_false = &right[W..W + len];
                        for i in 0..len {
                            cond[i] = where_value(cond[i], when_true[i], when_false[i]);
                        }
                    }
                }
            }
            out_chunk.copy_from_slice(&regs[..len]);
        });
    DagVal::Cells {
        values: Arc::new(out),
        layout,
    }
}

#[derive(Debug, Clone)]
enum DagVal {
    Cells {
        values: Arc<Vec<f64>>,
        layout: Layout,
    },
    Scalar(f64),
}

pub(crate) fn eval_exprs(exprs: &[Expr], cs: &CellSet) -> Result<Vec<Vec<f64>>> {
    let mut dag = Dag::default();
    let roots = exprs.iter().map(|expr| dag.lower(expr)).collect::<Vec<_>>();
    dag.fuse_single_use_transposes(&roots);
    dag.fuse_xs_output_transposes(&roots);
    dag.fuse_elementwise(&roots);
    let values = dag.eval_roots(&roots, cs)?;

    // Materializing each root (transpose to Tn + clone) is independent per
    // alpha, so fan it out across alphas instead of a serial map.
    Ok(exprs
        .par_iter()
        .zip(values)
        .map(|(_, value)| to_cells(&value, Layout::Tn, cs))
        .collect())
}

fn slot_value(slots: &[Option<Arc<DagVal>>], id: NodeId) -> &DagVal {
    slots[id.index()]
        .as_deref()
        .expect("child slot should be available")
}

fn eval_unary_elementwise(value: &DagVal, op: impl Fn(f64) -> f64) -> DagVal {
    match value {
        DagVal::Scalar(value) => DagVal::Scalar(op(*value)),
        DagVal::Cells { values, layout } => DagVal::Cells {
            values: Arc::new(values.iter().map(|value| op(*value)).collect()),
            layout: *layout,
        },
    }
}

fn eval_binary_elementwise(
    lhs: &DagVal,
    rhs: &DagVal,
    cs: &CellSet,
    op: impl Fn(f64, f64) -> f64,
) -> DagVal {
    match (lhs, rhs) {
        (DagVal::Scalar(lhs), DagVal::Scalar(rhs)) => DagVal::Scalar(op(*lhs, *rhs)),
        (
            DagVal::Cells {
                values: lhs_values,
                layout,
            },
            DagVal::Scalar(rhs),
        ) => DagVal::Cells {
            values: Arc::new(
                lhs_values
                    .iter()
                    .map(|lhs| op(*lhs, *rhs))
                    .collect::<Vec<_>>(),
            ),
            layout: *layout,
        },
        (
            DagVal::Scalar(lhs),
            DagVal::Cells {
                values: rhs_values,
                layout,
            },
        ) => DagVal::Cells {
            values: Arc::new(
                rhs_values
                    .iter()
                    .map(|rhs| op(*lhs, *rhs))
                    .collect::<Vec<_>>(),
            ),
            layout: *layout,
        },
        (
            DagVal::Cells {
                values: lhs_values,
                layout,
            },
            DagVal::Cells { .. },
        ) => {
            let rhs_values = cells_for(rhs, *layout, cs);
            DagVal::Cells {
                values: Arc::new(
                    lhs_values
                        .iter()
                        .zip(rhs_values.iter())
                        .map(|(lhs, rhs)| op(*lhs, *rhs))
                        .collect::<Vec<_>>(),
                ),
                layout: *layout,
            }
        }
    }
}

fn eval_cells_unary(
    value: &DagVal,
    want: Layout,
    output: Layout,
    cs: &CellSet,
    kernel: impl FnOnce(&[f64], &CellSet) -> Vec<f64>,
) -> DagVal {
    let values = cells_for(value, want, cs);
    DagVal::Cells {
        values: Arc::new(kernel(&values, cs)),
        layout: output,
    }
}

fn eval_cells_binary(
    lhs: &DagVal,
    rhs: &DagVal,
    want: Layout,
    output: Layout,
    cs: &CellSet,
    kernel: impl FnOnce(&[f64], &[f64], &CellSet) -> Vec<f64>,
) -> DagVal {
    let lhs = cells_for(lhs, want, cs);
    let rhs = cells_for(rhs, want, cs);
    DagVal::Cells {
        values: Arc::new(kernel(&lhs, &rhs, cs)),
        layout: output,
    }
}

/// Borrow a value's cells along with the layout they are stored in (a scalar is
/// broadcast as Nt cells). Cross-sectional kernels take this layout and read
/// their input in place, so an Nt input needs no transpose.
fn cells_and_layout<'a>(value: &'a DagVal, cs: &CellSet) -> (Cow<'a, [f64]>, Layout) {
    match value {
        DagVal::Cells { values, layout } => (Cow::Borrowed(values.as_slice()), *layout),
        DagVal::Scalar(value) => (Cow::Owned(vec![*value; cs.n_cells]), Layout::Nt),
    }
}

/// Cross-sectional unary op: read the input in its native layout and emit the
/// result directly in `output` (no follow-up transpose when `output` is Nt).
fn eval_xs_unary(
    value: &DagVal,
    output: Layout,
    cs: &CellSet,
    kernel: impl FnOnce(&[f64], Layout, Layout, &CellSet) -> Vec<f64>,
) -> DagVal {
    let (values, input_layout) = cells_and_layout(value, cs);
    DagVal::Cells {
        values: Arc::new(kernel(&values, input_layout, output, cs)),
        layout: output,
    }
}

/// Cross-sectional binary op (value + group key): each input is read in its own
/// native layout and the result is emitted directly in `output`.
fn eval_xs_binary(
    values: &DagVal,
    groups: &DagVal,
    output: Layout,
    cs: &CellSet,
    kernel: impl FnOnce(&[f64], Layout, &[f64], Layout, Layout, &CellSet) -> Vec<f64>,
) -> DagVal {
    let (values, values_layout) = cells_and_layout(values, cs);
    let (groups, groups_layout) = cells_and_layout(groups, cs);
    DagVal::Cells {
        values: Arc::new(kernel(
            &values,
            values_layout,
            &groups,
            groups_layout,
            output,
            cs,
        )),
        layout: output,
    }
}

fn eval_where(cond: &DagVal, when_true: &DagVal, when_false: &DagVal, cs: &CellSet) -> DagVal {
    let Some(layout) = first_value_layout(&[cond, when_true, when_false]) else {
        let DagVal::Scalar(cond) = cond else {
            unreachable!("all values are scalar");
        };
        let DagVal::Scalar(when_true) = when_true else {
            unreachable!("all values are scalar");
        };
        let DagVal::Scalar(when_false) = when_false else {
            unreachable!("all values are scalar");
        };
        return DagVal::Scalar(where_value(*cond, *when_true, *when_false));
    };

    let cond = cells_for(cond, layout, cs);
    let when_true = cells_for(when_true, layout, cs);
    let when_false = cells_for(when_false, layout, cs);
    DagVal::Cells {
        values: Arc::new(
            cond.iter()
                .zip(when_true.iter())
                .zip(when_false.iter())
                .map(|((cond, when_true), when_false)| where_value(*cond, *when_true, *when_false))
                .collect(),
        ),
        layout,
    }
}

fn first_value_layout(values: &[&DagVal]) -> Option<Layout> {
    values.iter().find_map(|value| match value {
        DagVal::Cells { layout, .. } => Some(*layout),
        DagVal::Scalar(_) => None,
    })
}

fn cells_for<'a>(value: &'a DagVal, want: Layout, cs: &CellSet) -> Cow<'a, [f64]> {
    match value {
        DagVal::Scalar(value) => Cow::Owned(vec![*value; cs.n_cells]),
        DagVal::Cells { values, layout } if *layout == want => Cow::Borrowed(values.as_slice()),
        DagVal::Cells {
            values,
            layout: Layout::Nt,
        } => Cow::Owned(nt_to_tn(values, cs)),
        DagVal::Cells {
            values,
            layout: Layout::Tn,
        } => Cow::Owned(tn_to_nt(values, cs)),
    }
}

fn to_cells(value: &DagVal, want: Layout, cs: &CellSet) -> Vec<f64> {
    match value {
        DagVal::Scalar(value) => vec![*value; cs.n_cells],
        DagVal::Cells { values, layout } if *layout == want => values.as_ref().clone(),
        DagVal::Cells {
            values,
            layout: Layout::Nt,
        } => nt_to_tn(values, cs),
        DagVal::Cells {
            values,
            layout: Layout::Tn,
        } => tn_to_nt(values, cs),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ops::Range;

    use polars::prelude::*;

    use super::*;
    use crate::alpha_eval::{eval, to_cells as tree_to_cells};

    fn test_cellset_fields(
        fields: HashMap<String, Vec<f64>>,
        sym_blocks: Vec<Range<usize>>,
        time_blocks: Vec<Range<usize>>,
        tn_order: Vec<usize>,
    ) -> CellSet {
        let n_cells = fields.values().next().map_or(0, Vec::len);
        CellSet {
            n_cells,
            sym_blocks,
            time_blocks,
            tn_order,
            orig_index_tn: (0..n_cells).collect(),
            fields: fields
                .into_iter()
                .map(|(name, values)| (name, Arc::new(values)))
                .collect(),
            symbols_tn: Column::new("asset".into(), vec!["A"; n_cells]),
            times_tn: Column::new("time".into(), (0..n_cells as i64).collect::<Vec<_>>()),
            time_block_by_value: HashMap::new(),
        }
    }

    fn eval_dag(expr: &Expr, cs: &CellSet) -> Result<Vec<f64>> {
        let mut dag = Dag::default();
        let root = dag.lower(expr);
        dag.fuse_single_use_transposes(&[root]);
        dag.fuse_xs_output_transposes(&[root]);
        dag.fuse_elementwise(&[root]);
        let values = dag.eval_roots(&[root], cs)?;
        Ok(to_cells(&values[0], Layout::Tn, cs))
    }

    fn eval_tree(expr: &Expr, cs: &CellSet) -> Result<Vec<f64>> {
        Ok(tree_to_cells(eval(expr, cs)?, Layout::Tn, cs).into_owned())
    }

    fn assert_vec_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (idx, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            if expected.is_nan() {
                assert!(actual.is_nan(), "idx {idx}: actual {actual}, expected NaN");
            } else if expected.is_infinite() {
                assert_eq!(
                    actual, expected,
                    "idx {idx}: actual {actual}, expected {expected}"
                );
            } else {
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "idx {idx}: actual {actual}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn hash_cons_deduplicates_repeated_transpose_subgraph() {
        let repeated = Expr::Rank(Box::new(Expr::Delay(
            Box::new(Expr::Field("close".to_string())),
            1,
        )));
        let expr = Expr::Add(Box::new(repeated.clone()), Box::new(repeated));
        let mut dag = Dag::default();

        let root = dag.lower(&expr);

        assert_eq!(dag.node_count(), 5);
        assert!(matches!(dag.nodes[root.index()], Node::Add(lhs, rhs) if lhs == rhs));
        assert!(
            dag.nodes
                .iter()
                .any(|node| matches!(node, Node::Transpose(_)))
        );
    }

    #[test]
    fn dag_eval_matches_tree_for_mixed_layout_expression() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([
                ("close".to_string(), vec![10.0, 12.0, 20.0, 24.0, 30.0]),
                ("volume".to_string(), vec![1.0, 2.0, 2.0, 1.0, 3.0]),
            ]),
            vec![0..2, 2..5],
            vec![0..2, 2..3, 3..5],
            vec![0, 2, 1, 3, 4],
        );
        let expr = Expr::Where(
            Box::new(Expr::Cmp(
                CmpOp::Gt,
                Box::new(Expr::TsMean(Box::new(Expr::Field("close".to_string())), 2)),
                Box::new(Expr::Delay(Box::new(Expr::Field("close".to_string())), 1)),
            )),
            Box::new(Expr::Rank(Box::new(Expr::Field("close".to_string())))),
            Box::new(Expr::Scale(
                Box::new(Expr::Field("volume".to_string())),
                1.0,
            )),
        );

        let actual = eval_dag(&expr, &cs)?;
        let expected = eval_tree(&expr, &cs)?;

        assert_vec_close(&actual, &expected);
        Ok(())
    }

    #[test]
    fn dag_eval_matches_tree_for_correlation_and_group_ops() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([
                ("close".to_string(), vec![1.0, 2.0, 3.0, 2.0, 4.0, 6.0]),
                ("volume".to_string(), vec![2.0, 3.0, 4.0, 4.0, 7.0, 8.0]),
                ("industry".to_string(), vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
            ]),
            vec![0..3, 3..6],
            vec![0..2, 2..4, 4..6],
            vec![0, 3, 1, 4, 2, 5],
        );
        let expr = Expr::GroupNeutralize(
            Box::new(Expr::Correlation(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Field("volume".to_string())),
                2,
            )),
            Box::new(Expr::Field("industry".to_string())),
        );

        let actual = eval_dag(&expr, &cs)?;
        let expected = eval_tree(&expr, &cs)?;

        assert_vec_close(&actual, &expected);
        Ok(())
    }

    #[test]
    fn dag_eval_matches_tree_for_regression_and_quantile_ops() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([("close".to_string(), vec![1.0, 3.0, 5.0, 7.0, 8.0])]),
            std::iter::once(0..5).collect(),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
            vec![0, 1, 2, 3, 4],
        );
        let expr = Expr::Add(
            Box::new(Expr::Slope(Box::new(Expr::Field("close".to_string())), 3)),
            Box::new(Expr::Add(
                Box::new(Expr::Rsquare(Box::new(Expr::Field("close".to_string())), 3)),
                Box::new(Expr::Add(
                    Box::new(Expr::Resi(Box::new(Expr::Field("close".to_string())), 3)),
                    Box::new(Expr::Quantile(
                        Box::new(Expr::Field("close".to_string())),
                        3,
                        0.8,
                    )),
                )),
            )),
        );

        let actual = eval_dag(&expr, &cs)?;
        let expected = eval_tree(&expr, &cs)?;

        assert_vec_close(&actual, &expected);
        Ok(())
    }

    #[test]
    fn dag_eval_matches_tree_for_deep_elementwise_chain() -> Result<()> {
        // Ten single-use fields collapse into one deep FusedEw program that
        // exercises every EwOp (both binary and unary), a ternary Where, and a
        // register depth > 2 — so the columnar interpreter's register stack,
        // split-borrow, and Where's two-pop unwind all run. The panel mixes
        // negatives and zeros so Log/SignedPower/Sign/Div produce NaN/inf lanes.
        let cs = test_cellset_fields(
            HashMap::from([
                ("a".to_string(), vec![2.0, -3.0, 0.0, 4.0, -1.0, 5.0]),
                ("b".to_string(), vec![1.0, 3.0, -2.0, 4.0, 1.0, -5.0]),
                ("c".to_string(), vec![2.0, 0.0, 3.0, -1.0, 2.0, 4.0]),
                ("d".to_string(), vec![-1.0, 2.0, 0.0, 3.0, -4.0, 1.0]),
                ("e".to_string(), vec![0.0, 1.0, 2.0, -3.0, 4.0, -5.0]),
                ("f".to_string(), vec![3.0, -2.0, 1.0, 0.0, 2.0, -1.0]),
                ("g".to_string(), vec![1.0, 1.0, -1.0, 2.0, -2.0, 3.0]),
                ("h".to_string(), vec![4.0, -4.0, 2.0, -2.0, 0.0, 1.0]),
                ("i".to_string(), vec![-3.0, 3.0, 1.0, -1.0, 5.0, 0.0]),
                ("j".to_string(), vec![2.0, 2.0, -2.0, 0.0, 3.0, -3.0]),
            ]),
            vec![0..3, 3..6],
            vec![0..2, 2..4, 4..6],
            vec![0, 3, 1, 4, 2, 5],
        );
        let field = |name: &str| Box::new(Expr::Field(name.to_string()));
        let expr = Expr::Add(
            Box::new(Expr::Div(
                Box::new(Expr::Log(Box::new(Expr::Abs(Box::new(Expr::Sub(
                    field("a"),
                    field("b"),
                )))))),
                field("c"),
            )),
            Box::new(Expr::Where(
                Box::new(Expr::Cmp(
                    CmpOp::Gt,
                    Box::new(Expr::Sign(field("d"))),
                    field("e"),
                )),
                Box::new(Expr::SignedPower(field("f"), Box::new(Expr::Const(0.5)))),
                Box::new(Expr::Max(
                    Box::new(Expr::Neg(Box::new(Expr::Mul(field("g"), field("h"))))),
                    Box::new(Expr::Min(
                        field("i"),
                        Box::new(Expr::Power(field("j"), Box::new(Expr::Const(2.0)))),
                    )),
                )),
            )),
        );

        let actual = eval_dag(&expr, &cs)?;
        let expected = eval_tree(&expr, &cs)?;

        assert_vec_close(&actual, &expected);
        Ok(())
    }
}
