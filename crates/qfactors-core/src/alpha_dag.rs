use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;

use crate::alpha_eval::{
    cmp_value, correlation, covariance, decay_linear, delay, delta, group_neutralize, group_rank,
    log_value, max_value, min_value, product, rank, scale, sign, signed_power, ts_argmax,
    ts_argmin, ts_max, ts_mean, ts_min, ts_rank, ts_std, ts_sum, where_value,
};
use crate::cellset::CellSet;
use crate::error::{QFactorsError, Result};
use crate::expr::{CmpOp, Expr};
use crate::layout::{Layout, nt_to_tn, tn_to_nt};

/// Upper bound on nodes evaluated concurrently within one dependency level.
/// Bounds peak memory (one full-panel output per in-flight node) while staying a
/// small multiple of the core count, so every core stays busy and heavy nodes
/// still parallelize their own kernel via work-stealing.
fn max_nodes_in_flight() -> usize {
    2 * std::thread::available_parallelism().map_or(8, |n| n.get())
}

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
    TsStd(NodeId, usize),
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
            | Node::TsStd(inner, _)
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
            Expr::TsStd(inner, days) => self.lower_ts_unary(inner, *days, Node::TsStd),
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
                    .map(|&id| self.eval_node(id, &slots, cs).map(|value| (id, Arc::new(value))))
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

    fn eval_node(&self, id: NodeId, slots: &[Option<Arc<DagVal>>], cs: &CellSet) -> Result<DagVal> {
        Ok(match self.nodes[id.index()] {
            Node::Field(ref name) => {
                let values = cs
                    .fields
                    .get(name)
                    .ok_or_else(|| QFactorsError::MissingColumn(name.clone()))?;
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
            Node::TsStd(inner, days) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Nt,
                Layout::Nt,
                cs,
                |values, cs| ts_std(values, days, cs),
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
                eval_cells_unary(slot_value(slots, inner), Layout::Tn, Layout::Tn, cs, rank)
            }
            Node::Scale(inner, scale_to) => eval_cells_unary(
                slot_value(slots, inner),
                Layout::Tn,
                Layout::Tn,
                cs,
                |values, cs| scale(values, f64::from_bits(scale_to), cs),
            ),
            Node::GroupRank(values, groups) => eval_cells_binary(
                slot_value(slots, values),
                slot_value(slots, groups),
                Layout::Tn,
                Layout::Tn,
                cs,
                group_rank,
            ),
            Node::GroupNeutralize(values, groups) => eval_cells_binary(
                slot_value(slots, values),
                slot_value(slots, groups),
                Layout::Tn,
                Layout::Tn,
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
        })
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

pub(crate) fn eval_alphas(
    resolved: &[(String, Expr)],
    cs: &CellSet,
) -> Result<Vec<(String, Vec<f64>)>> {
    let mut dag = Dag::default();
    let roots = resolved
        .iter()
        .map(|(_, expr)| dag.lower(expr))
        .collect::<Vec<_>>();
    let values = dag.eval_roots(&roots, cs)?;

    // Materializing each root (transpose to Tn + clone) is independent per
    // alpha, so fan it out across alphas instead of a serial map.
    Ok(resolved
        .par_iter()
        .zip(values)
        .map(|((name, _), value)| (name.clone(), to_cells(&value, Layout::Tn, cs)))
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
        let values = dag.eval_roots(&[root], cs)?;
        Ok(to_cells(&values[0], Layout::Tn, cs))
    }

    fn eval_tree(expr: &Expr, cs: &CellSet) -> Result<Vec<f64>> {
        Ok(tree_to_cells(eval(expr, cs)?, Layout::Tn, cs))
    }

    fn assert_vec_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (idx, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            if expected.is_nan() {
                assert!(actual.is_nan(), "idx {idx}: actual {actual}, expected NaN");
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
}
