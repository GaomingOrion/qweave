use std::collections::BTreeSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Error, FnArg, GenericArgument, Ident, ItemFn, Lit, LitStr, Pat, PathArguments, ReturnType,
    Token, Type, TypePath, braced, bracketed,
};

#[proc_macro_attribute]
pub fn factor(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_factor(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_factor(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let args: FactorArgs = syn::parse2(attr)?;
    let function: ItemFn = syn::parse2(item)?;
    let analysis = FunctionAnalysis::new(&function, &args)?;

    Ok(generate_factor(function, analysis))
}

enum WindowArgs {
    Single(usize),
    Multi(Vec<usize>),
}

impl WindowArgs {
    fn values(&self) -> Vec<usize> {
        match self {
            Self::Single(window) => vec![*window],
            Self::Multi(windows) => windows.clone(),
        }
    }

    fn is_multi(&self) -> bool {
        matches!(self, Self::Multi(_))
    }
}

struct FactorArgs {
    windows: WindowArgs,
    outputs: Option<Vec<String>>,
    params: Option<Vec<ParamSet>>,
}

struct ParamSet {
    name: String,
    values: Vec<ParamValueSpec>,
}

struct NormalizedParamSet {
    name: Option<String>,
    values: Vec<ParamValueSpec>,
}

#[derive(Clone)]
struct ParamValueSpec {
    name: String,
    value: f64,
}

impl Parse for FactorArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut window = None;
        let mut windows = None;
        let mut outputs = None;
        let mut params = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "window" {
                if window.is_some() {
                    return Err(Error::new_spanned(key, "duplicate `window`"));
                }
                window = Some(parse_window_literal(input)?);
            } else if key == "windows" {
                if windows.is_some() {
                    return Err(Error::new_spanned(key, "duplicate `windows`"));
                }
                windows = Some(parse_windows(input)?);
            } else if key == "outputs" {
                if outputs.is_some() {
                    return Err(Error::new_spanned(key, "duplicate `outputs`"));
                }
                outputs = Some(parse_outputs(input)?);
            } else if key == "params" {
                if params.is_some() {
                    return Err(Error::new_spanned(key, "duplicate `params`"));
                }
                params = Some(parse_params(input)?);
            } else {
                return Err(Error::new_spanned(key, "unsupported factor attribute"));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else if !input.is_empty() {
                return Err(input.error("expected `,` between factor attributes"));
            }
        }

        let windows = match (window, windows) {
            (Some(window), None) => WindowArgs::Single(window),
            (None, Some(windows)) => WindowArgs::Multi(windows),
            (Some(_), Some(_)) => {
                return Err(Error::new(
                    proc_macro2::Span::call_site(),
                    "`window` and `windows` cannot be used together",
                ));
            }
            (None, None) => {
                return Err(Error::new(
                    proc_macro2::Span::call_site(),
                    "expected `window = N` or `windows = [N, ...]`",
                ));
            }
        };

        Ok(Self {
            windows,
            outputs,
            params,
        })
    }
}

fn parse_window_literal(input: ParseStream<'_>) -> syn::Result<usize> {
    let lit: Lit = input.parse()?;
    let Lit::Int(lit) = lit else {
        return Err(Error::new_spanned(
            lit,
            "`window` must be a positive integer",
        ));
    };

    let value = lit.base10_parse::<usize>()?;
    if value == 0 {
        return Err(Error::new_spanned(
            lit,
            "`window` must be greater than zero",
        ));
    }
    Ok(value)
}

fn parse_windows(input: ParseStream<'_>) -> syn::Result<Vec<usize>> {
    let content;
    bracketed!(content in input);

    let mut windows = Vec::new();
    let mut seen = BTreeSet::new();
    while !content.is_empty() {
        let window = parse_window_literal(&content)?;
        if !seen.insert(window) {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "`windows` contains a duplicate window",
            ));
        }
        windows.push(window);

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if !content.is_empty() {
            return Err(content.error("expected `,` between windows"));
        }
    }

    if windows.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "`windows` cannot be empty",
        ));
    }
    Ok(windows)
}

fn parse_outputs(input: ParseStream<'_>) -> syn::Result<Vec<String>> {
    let content;
    bracketed!(content in input);

    let mut outputs = Vec::new();
    while !content.is_empty() {
        let lit: LitStr = content.parse()?;
        let value = lit.value();
        if value.is_empty() {
            return Err(Error::new_spanned(lit, "`outputs` entries cannot be empty"));
        }
        outputs.push(value);

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if !content.is_empty() {
            return Err(content.error("expected `,` between outputs"));
        }
    }

    if outputs.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "`outputs` cannot be empty",
        ));
    }
    Ok(outputs)
}

fn parse_params(input: ParseStream<'_>) -> syn::Result<Vec<ParamSet>> {
    let content;
    bracketed!(content in input);

    let mut sets = Vec::new();
    let mut names = BTreeSet::new();
    while !content.is_empty() {
        let set = parse_param_set(&content)?;
        if !names.insert(set.name.clone()) {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "`params` contains a duplicate parameter set name",
            ));
        }
        sets.push(set);

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if !content.is_empty() {
            return Err(content.error("expected `,` between parameter sets"));
        }
    }

    if sets.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "`params` cannot be empty",
        ));
    }
    Ok(sets)
}

fn parse_param_set(input: ParseStream<'_>) -> syn::Result<ParamSet> {
    let content;
    braced!(content in input);

    let mut set_name = None;
    let mut values = Vec::new();
    let mut value_names = BTreeSet::new();

    while !content.is_empty() {
        let key: Ident = content.parse()?;
        content.parse::<Token![=]>()?;

        if key == "name" {
            if set_name.is_some() {
                return Err(Error::new_spanned(key, "duplicate parameter set `name`"));
            }
            let lit: LitStr = content.parse()?;
            let value = lit.value();
            if value.is_empty() {
                return Err(Error::new_spanned(
                    lit,
                    "parameter set `name` cannot be empty",
                ));
            }
            set_name = Some(value);
        } else {
            let value = parse_f64_literal(&content)?;
            let name = key.to_string();
            if !value_names.insert(name.clone()) {
                return Err(Error::new_spanned(key, "duplicate parameter value"));
            }
            values.push(ParamValueSpec { name, value });
        }

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if !content.is_empty() {
            return Err(content.error("expected `,` between parameter entries"));
        }
    }

    let Some(name) = set_name else {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "parameter set requires `name = \"...\"`",
        ));
    };
    if values.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "parameter set must define at least one f64 parameter",
        ));
    }

    Ok(ParamSet { name, values })
}

fn parse_f64_literal(input: ParseStream<'_>) -> syn::Result<f64> {
    let lit: Lit = input.parse()?;
    match lit {
        Lit::Float(lit) => lit.base10_parse::<f64>(),
        Lit::Int(lit) => lit.base10_parse::<f64>(),
        other => Err(Error::new_spanned(
            other,
            "factor params only support f64 numeric literals",
        )),
    }
}

struct FunctionAnalysis {
    kernel_name: String,
    args: Vec<ArgumentSpec>,
    inputs: Vec<InputSpec>,
    params: Vec<ParamSpec>,
    output_names: Vec<String>,
    output_count: usize,
    returns_result: bool,
    factors: Vec<GeneratedFactor>,
}

enum ArgumentSpec {
    Input(usize),
    Param(usize),
}

struct InputSpec {
    name: String,
    dtype: Ident,
    accessor: Ident,
}

struct ParamSpec {
    name: String,
}

struct GeneratedFactor {
    factor_name: String,
    window: usize,
    param_set: Option<String>,
    params: Vec<ParamValueSpec>,
    params_ident: Option<Ident>,
    descriptor_ident: Ident,
    register_ident: Ident,
}

enum OutputShape {
    Single,
    Tuple(usize),
}

struct ReturnSpec {
    shape: OutputShape,
    returns_result: bool,
}

impl FunctionAnalysis {
    fn new(function: &ItemFn, args: &FactorArgs) -> syn::Result<Self> {
        let signature = &function.sig;
        if signature.constness.is_some() {
            return Err(Error::new_spanned(
                signature.constness,
                "factor functions cannot be const",
            ));
        }
        if signature.asyncness.is_some() {
            return Err(Error::new_spanned(
                signature.asyncness,
                "factor functions cannot be async",
            ));
        }
        if signature.unsafety.is_some() {
            return Err(Error::new_spanned(
                signature.unsafety,
                "factor functions cannot be unsafe",
            ));
        }
        if !signature.generics.params.is_empty() {
            return Err(Error::new_spanned(
                &signature.generics,
                "factor functions cannot be generic",
            ));
        }
        if signature.variadic.is_some() {
            return Err(Error::new_spanned(
                &signature.variadic,
                "factor functions cannot be variadic",
            ));
        }

        let mut arg_specs = Vec::with_capacity(signature.inputs.len());
        let mut inputs = Vec::new();
        let mut params = Vec::new();
        for arg in &signature.inputs {
            match parse_argument(arg)? {
                ParsedArgument::Input(input) => {
                    let idx = inputs.len();
                    inputs.push(input);
                    arg_specs.push(ArgumentSpec::Input(idx));
                }
                ParsedArgument::Param(param) => {
                    let idx = params.len();
                    params.push(param);
                    arg_specs.push(ArgumentSpec::Param(idx));
                }
            }
        }

        let param_sets = validate_param_sets(&params, args.params.as_ref(), &signature.ident)?;
        let return_spec = parse_return_type(&signature.output)?;
        let (output_names, output_count) =
            resolve_output_names(&signature.ident, args.outputs.as_ref(), &return_spec.shape)?;
        let factors = factor_names(&signature.ident, &args.windows, &param_sets)?;

        Ok(Self {
            kernel_name: signature.ident.to_string(),
            args: arg_specs,
            inputs,
            params,
            output_names,
            output_count,
            returns_result: return_spec.returns_result,
            factors,
        })
    }
}

enum ParsedArgument {
    Input(InputSpec),
    Param(ParamSpec),
}

fn parse_argument(arg: &FnArg) -> syn::Result<ParsedArgument> {
    let FnArg::Typed(typed) = arg else {
        return Err(Error::new_spanned(arg, "factor methods are not supported"));
    };

    let Pat::Ident(pattern) = typed.pat.as_ref() else {
        return Err(Error::new_spanned(
            &typed.pat,
            "factor input must use an identifier pattern",
        ));
    };
    if pattern.subpat.is_some() {
        return Err(Error::new_spanned(
            pattern,
            "factor input must use a plain identifier",
        ));
    }

    if is_f64_type(&typed.ty) {
        return Ok(ParsedArgument::Param(ParamSpec {
            name: pattern.ident.to_string(),
        }));
    }

    let (dtype, accessor) = parse_slice_type(&typed.ty)?;
    Ok(ParsedArgument::Input(InputSpec {
        name: pattern.ident.to_string(),
        dtype,
        accessor,
    }))
}

fn parse_slice_type(ty: &Type) -> syn::Result<(Ident, Ident)> {
    let Type::Reference(reference) = ty else {
        return Err(Error::new_spanned(
            ty,
            "factor inputs must be typed slices like `&[f64]` or f64 compile-time params",
        ));
    };
    if reference.mutability.is_some() {
        return Err(Error::new_spanned(
            reference,
            "factor inputs cannot be mutable",
        ));
    }

    let Type::Slice(slice) = reference.elem.as_ref() else {
        return Err(Error::new_spanned(
            &reference.elem,
            "factor inputs must be typed slices like `&[f64]`",
        ));
    };

    let Type::Path(TypePath { qself: None, path }) = slice.elem.as_ref() else {
        return Err(Error::new_spanned(
            &slice.elem,
            "factor input dtype must be `f64`, `u32`, or `i64`",
        ));
    };
    let Some(ident) = path.get_ident() else {
        return Err(Error::new_spanned(
            path,
            "factor input dtype must be `f64`, `u32`, or `i64`",
        ));
    };

    match ident.to_string().as_str() {
        "f64" => Ok((format_ident!("F64"), format_ident!("f64"))),
        "u32" => Ok((format_ident!("U32"), format_ident!("u32"))),
        "i64" => Ok((format_ident!("I64"), format_ident!("i64"))),
        _ => Err(Error::new_spanned(
            ident,
            "factor input dtype must be `f64`, `u32`, or `i64`",
        )),
    }
}

fn validate_param_sets(
    params: &[ParamSpec],
    attr_sets: Option<&Vec<ParamSet>>,
    kernel_ident: &Ident,
) -> syn::Result<Vec<NormalizedParamSet>> {
    match (params.is_empty(), attr_sets) {
        (true, None) => Ok(vec![NormalizedParamSet {
            name: None,
            values: Vec::new(),
        }]),
        (true, Some(_)) => Err(Error::new_spanned(
            kernel_ident,
            "`params` requires at least one f64 kernel parameter",
        )),
        (false, None) => Err(Error::new_spanned(
            kernel_ident,
            "f64 kernel parameters require `params = [...]`",
        )),
        (false, Some(sets)) => {
            let expected = params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<BTreeSet<_>>();
            let mut normalized_sets = Vec::with_capacity(sets.len());

            for set in sets {
                let actual = set
                    .values
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<BTreeSet<_>>();
                if actual != expected {
                    return Err(Error::new_spanned(
                        kernel_ident,
                        "each parameter set must match the f64 kernel parameter names",
                    ));
                }

                let mut values = Vec::with_capacity(params.len());
                for param in params {
                    let value = set
                        .values
                        .iter()
                        .find(|value| value.name == param.name)
                        .expect("validated parameter set names")
                        .clone();
                    values.push(value);
                }
                normalized_sets.push(NormalizedParamSet {
                    name: Some(set.name.clone()),
                    values,
                });
            }

            Ok(normalized_sets)
        }
    }
}

fn parse_return_type(output: &ReturnType) -> syn::Result<ReturnSpec> {
    let ReturnType::Type(_, ty) = output else {
        return Err(Error::new_spanned(
            output,
            "factor functions must return `f64`, `(f64, ...)`, or `Result<T>`",
        ));
    };

    if let Some(inner) = result_inner(ty)? {
        let shape = parse_output_shape(inner)?;
        Ok(ReturnSpec {
            shape,
            returns_result: true,
        })
    } else {
        let shape = parse_output_shape(ty)?;
        Ok(ReturnSpec {
            shape,
            returns_result: false,
        })
    }
}

fn result_inner(ty: &Type) -> syn::Result<Option<&Type>> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return Ok(None);
    };
    let Some(segment) = path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "Result" {
        return Ok(None);
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(Error::new_spanned(
            ty,
            "`Result` return type must be `Result<T>`",
        ));
    };
    if args.args.len() != 1 {
        return Err(Error::new_spanned(
            ty,
            "`Result` return type must be `Result<T>`",
        ));
    }

    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return Err(Error::new_spanned(
            ty,
            "`Result` return type must be `Result<T>`",
        ));
    };
    Ok(Some(inner))
}

fn parse_output_shape(ty: &Type) -> syn::Result<OutputShape> {
    if is_f64_type(ty) {
        return Ok(OutputShape::Single);
    }

    let Type::Tuple(tuple) = ty else {
        return Err(Error::new_spanned(
            ty,
            "factor return type must be `f64`, `(f64, ...)`, or `Result<T>`",
        ));
    };
    if tuple.elems.len() < 2 {
        return Err(Error::new_spanned(
            tuple,
            "tuple factor output must contain at least two values",
        ));
    }
    for elem in &tuple.elems {
        if !is_f64_type(elem) {
            return Err(Error::new_spanned(
                elem,
                "factor outputs must be `f64` values",
            ));
        }
    }

    Ok(OutputShape::Tuple(tuple.elems.len()))
}

fn is_f64_type(ty: &Type) -> bool {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return false;
    };
    path.is_ident("f64")
}

fn resolve_output_names(
    kernel_ident: &Ident,
    attr_outputs: Option<&Vec<String>>,
    shape: &OutputShape,
) -> syn::Result<(Vec<String>, usize)> {
    match shape {
        OutputShape::Single => {
            let output_names = attr_outputs
                .cloned()
                .unwrap_or_else(|| vec![kernel_ident.to_string()]);
            if output_names.len() != 1 {
                return Err(Error::new_spanned(
                    kernel_ident,
                    "single-output factors must have exactly one output name",
                ));
            }
            Ok((output_names, 1))
        }
        OutputShape::Tuple(count) => {
            let Some(output_names) = attr_outputs.cloned() else {
                return Err(Error::new_spanned(
                    kernel_ident,
                    "multi-output factors must define `outputs = [...]`",
                ));
            };
            if output_names.len() != *count {
                return Err(Error::new_spanned(
                    kernel_ident,
                    "`outputs` length must match tuple return length",
                ));
            }
            Ok((output_names, *count))
        }
    }
}

fn factor_names(
    kernel_ident: &Ident,
    windows: &WindowArgs,
    param_sets: &[NormalizedParamSet],
) -> syn::Result<Vec<GeneratedFactor>> {
    let kernel_name = kernel_ident.to_string();
    let window_values = windows.values();
    let mut generated = Vec::with_capacity(window_values.len() * param_sets.len());
    let mut seen_names = BTreeSet::new();

    for (window_idx, window) in window_values.iter().enumerate() {
        for (param_idx, param_set) in param_sets.iter().enumerate() {
            let factor_name = factor_name_for(
                &kernel_name,
                windows.is_multi(),
                *window,
                param_set.name.as_deref(),
            );
            if !seen_names.insert(factor_name.clone()) {
                return Err(Error::new(
                    proc_macro2::Span::call_site(),
                    "factor macro expands to duplicate factor names",
                ));
            }

            let factor_idx = generated.len();
            generated.push(GeneratedFactor {
                factor_name,
                window: *window,
                param_set: param_set.name.clone(),
                params: param_set.values.clone(),
                params_ident: if param_set.values.is_empty() {
                    None
                } else {
                    Some(format_ident!(
                        "__qfactors_{}_{}_{}_params",
                        kernel_ident,
                        window_idx,
                        param_idx
                    ))
                },
                descriptor_ident: format_ident!(
                    "__qfactors_{}_{}_descriptor",
                    kernel_ident,
                    factor_idx
                ),
                register_ident: format_ident!(
                    "__QFACTORS_REGISTER_{}_{}",
                    kernel_name.to_ascii_uppercase(),
                    factor_idx
                ),
            });
        }
    }

    Ok(generated)
}

fn factor_name_for(
    kernel_name: &str,
    multi_window: bool,
    window: usize,
    param_set: Option<&str>,
) -> String {
    match (multi_window, param_set) {
        (false, None) => kernel_name.to_string(),
        (true, None) => format!("{kernel_name}_{window}"),
        (false, Some(param_set)) => format!("{kernel_name}_{param_set}"),
        (true, Some(param_set)) => format!("{kernel_name}_{window}_{param_set}"),
    }
}

fn generate_factor(function: ItemFn, analysis: FunctionAnalysis) -> TokenStream2 {
    let kernel_ident = &function.sig.ident;
    let inputs_ident = format_ident!("__qfactors_{}_inputs", kernel_ident);
    let outputs_ident = format_ident!("__qfactors_{}_outputs", kernel_ident);
    let compute_ident = format_ident!("__qfactors_{}_compute", kernel_ident);

    let input_count = analysis.inputs.len();
    let input_names = analysis.inputs.iter().map(|input| &input.name);
    let input_dtypes = analysis.inputs.iter().map(|input| &input.dtype);
    let output_count = analysis.output_count;
    let output_names = analysis.output_names.iter();

    let output_indices = 0..analysis.output_count;
    let output_vecs = (0..analysis.output_count)
        .map(|idx| format_ident!("__qfactors_output_{idx}"))
        .collect::<Vec<_>>();
    let output_values = (0..analysis.output_count)
        .map(|idx| format_ident!("__qfactors_value_{idx}"))
        .collect::<Vec<_>>();
    let input_locals = (0..analysis.inputs.len())
        .map(|idx| format_ident!("__qfactors_input_{idx}"))
        .collect::<Vec<_>>();
    let input_accessors = analysis.inputs.iter().map(|input| &input.accessor);
    let input_indices = 0..analysis.inputs.len();
    let param_locals = (0..analysis.params.len())
        .map(|idx| format_ident!("__qfactors_param_{idx}"))
        .collect::<Vec<_>>();
    let param_indices = 0..analysis.params.len();

    let call_args = analysis.args.iter().map(|arg| match arg {
        ArgumentSpec::Input(idx) => {
            let ident = &input_locals[*idx];
            quote! { &#ident[__qfactors_range.clone()] }
        }
        ArgumentSpec::Param(idx) => {
            let ident = &param_locals[*idx];
            quote! { #ident }
        }
    });
    let kernel_call = quote! { #kernel_ident(#(#call_args),*) };
    let kernel_call = if analysis.returns_result {
        quote! { #kernel_call? }
    } else {
        kernel_call
    };

    let assign_outputs = if analysis.output_count == 1 {
        let output_vec = &output_vecs[0];
        quote! {
            #output_vec[__qfactors_group_idx] = __qfactors_kernel_output;
        }
    } else {
        quote! {
            let (#(#output_values),*) = __qfactors_kernel_output;
            #(
                #output_vecs[__qfactors_group_idx] = #output_values;
            )*
        }
    };

    let param_statics = analysis.factors.iter().filter_map(|factor| {
        let params_ident = factor.params_ident.as_ref()?;
        let param_count = factor.params.len();
        let param_names = factor.params.iter().map(|param| &param.name);
        let param_values = factor.params.iter().map(|param| param.value);
        Some(quote! {
            #[allow(non_upper_case_globals)]
            static #params_ident: [::qfactors_core::ParamSpec; #param_count] = [
                #(
                    ::qfactors_core::ParamSpec {
                        name: #param_names,
                        value: ::qfactors_core::ParamValue::F64(#param_values),
                    },
                )*
            ];
        })
    });

    let descriptor_fns = analysis.factors.iter().map(|factor| {
        let descriptor_ident = &factor.descriptor_ident;
        let register_ident = &factor.register_ident;
        let factor_name = &factor.factor_name;
        let kernel_name = &analysis.kernel_name;
        let window = factor.window;
        let param_set = match &factor.param_set {
            Some(param_set) => quote! { Some(#param_set) },
            None => quote! { None },
        };
        let params = match &factor.params_ident {
            Some(params_ident) => quote! { &#params_ident },
            None => quote! { &[] },
        };

        quote! {
            fn #descriptor_ident() -> ::qfactors_core::FactorDescriptor {
                ::qfactors_core::FactorDescriptor {
                    factor_name: #factor_name,
                    kernel_name: #kernel_name,
                    window: #window,
                    inputs: &#inputs_ident,
                    outputs: &#outputs_ident,
                    param_set: #param_set,
                    params: #params,
                    compute: #compute_ident,
                }
            }

            #[linkme::distributed_slice(qfactors_core::registry::FACTOR_DESCRIPTORS)]
            static #register_ident: fn() -> ::qfactors_core::FactorDescriptor = #descriptor_ident;
        }
    });

    quote! {
        #function

        #[allow(non_upper_case_globals)]
        static #inputs_ident: [::qfactors_core::ColumnSpec; #input_count] = [
            #(
                ::qfactors_core::ColumnSpec {
                    name: #input_names,
                    dtype: ::qfactors_core::DType::#input_dtypes,
                },
            )*
        ];

        #[allow(non_upper_case_globals)]
        static #outputs_ident: [::qfactors_core::ColumnSpec; #output_count] = [
            #(
                ::qfactors_core::ColumnSpec {
                    name: #output_names,
                    dtype: ::qfactors_core::DType::F64,
                },
            )*
        ];

        #(#param_statics)*

        fn #compute_ident(
            columns: &::qfactors_core::ColumnStore<'_>,
            ranges: &[::std::option::Option<::std::ops::Range<usize>>],
            factor: &::qfactors_core::ResolvedFactor<'_>,
        ) -> ::qfactors_core::Result<::qfactors_core::FactorResult> {
            #(
                let #input_locals = columns.#input_accessors(&factor.input_columns[#input_indices])?;
            )*
            #(
                let #param_locals = match factor.desc.params[#param_indices].value {
                    ::qfactors_core::ParamValue::F64(value) => value,
                };
            )*
            #(
                let mut #output_vecs = vec![f64::NAN; ranges.len()];
            )*

            for (__qfactors_group_idx, __qfactors_range_opt) in ranges.iter().enumerate() {
                if let Some(__qfactors_range) = __qfactors_range_opt {
                    let __qfactors_kernel_output = #kernel_call;
                    #assign_outputs
                }
            }

            Ok(vec![
                #(
                    ::polars::prelude::Column::new(
                        factor.output_columns[#output_indices].clone().into(),
                        #output_vecs,
                    ),
                )*
            ])
        }

        #(#descriptor_fns)*
    }
}
