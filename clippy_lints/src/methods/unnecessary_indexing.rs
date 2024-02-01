use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::{snippet, snippet_with_applicability};
use clippy_utils::sugg::Sugg;
use clippy_utils::visitors::for_each_expr_with_closures;
use clippy_utils::{consts, higher, path_to_local};
use rustc_ast::UnOp;
use rustc_errors::Applicability;
use rustc_hir::{Block, Expr, ExprKind, HirId, Local, Node};
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

use std::ops::ControlFlow;

use super::UNNECESSARY_INDEXING;

struct IfExprWithIsEmpty<'hir> {
    higher_if: higher::If<'hir>,
    /// Indicates whether this `if` expr is checking `is_empty` or not.
    ///
    /// For example,
    /// `if x.is_empty()` is `true`, `if !x.is_empty()` will be false, etc.
    if_is_empty: bool,
}

impl<'hir> IfExprWithIsEmpty<'hir> {
    fn new_with_not_op_count(higher_if: higher::If<'hir>, not_op_count: u32) -> Self {
        Self {
            higher_if,
            if_is_empty: not_op_count % 2 == 0,
        }
    }

    /// Return the `Block` to visit after assuming the condition.
    ///
    /// Meaning that if `if_is_empty` is `false`, the `then` block will be returned,
    /// otherwise the `else` block will be returned.
    ///
    /// i.e. In this following example, the `if_is_empty` will be false,
    /// thus returning the block containing `// do something`
    ///
    /// ```no_run
    /// if !seq.is_empty() {
    ///     // do something
    /// } else {
    ///     // do some other thing
    /// }
    /// ```
    fn block_to_visit(&self) -> Option<&'hir Block<'hir>> {
        let get_block_from_expr_opt = |opt: Option<&Expr<'hir>>| -> Option<&Block<'hir>> {
            opt.and_then(|ex| {
                if let ExprKind::Block(b, _) = ex.kind {
                    Some(b)
                } else {
                    None
                }
            })
        };

        if self.if_is_empty {
            get_block_from_expr_opt(self.higher_if.r#else)
        } else {
            get_block_from_expr_opt(Some(self.higher_if.then))
        }
    }
}

pub(super) fn check(cx: &LateContext<'_>, expr: &Expr<'_>, method_name: Symbol, receiver: &Expr<'_>) {
    if method_name.as_str() == "is_empty"
        && let Some(parent_if) = get_higher_if(cx, expr.hir_id)
        && let Some(block) = parent_if.block_to_visit()
    {
        let mut should_lint = false;
        let mut spans_to_replace: Vec<Span> = vec![];

        // Visit the block to search for `receiver[0]` and other index expr.
        for_each_expr_with_closures(cx, block, |ex| {
            match ex.kind {
                ExprKind::Index(seq, bracket, _) => {
                    if let Some(seq_path_hid) = path_to_local(seq)
                        && let Some(recv_path_hid) = path_to_local(receiver)
                        && seq_path_hid == recv_path_hid
                        && let Some(consts::FullInt::U(val)) =
                            consts::constant_full_int(cx, cx.typeck_results(), bracket)
                    {
                        if val == 0 {
                            should_lint = true;
                            spans_to_replace.push(ex.span);
                        } else {
                            should_lint = false;
                            return ControlFlow::Break(());
                        }
                    }
                },
                _ => (),
            }
            ControlFlow::Continue(())
        });

        if !should_lint {
            return;
        }

        span_lint_and_then(
            cx,
            UNNECESSARY_INDEXING,
            parent_if.higher_if.cond.span,
            "this if condition could be replaced with if-let pettern with `.first()`",
            |diag| {},
        );
    }
}

fn get_higher_if<'hir>(cx: &LateContext<'hir>, hir_id: HirId) -> Option<IfExprWithIsEmpty<'hir>> {
    let mut not_op_count: u32 = 0;

    for (_, node) in cx.tcx.hir().parent_iter(hir_id) {
        let Node::Expr(expr) = node else { return None };

        if let Some(parent_if) = higher::If::hir(expr) {
            return Some(IfExprWithIsEmpty::new_with_not_op_count(parent_if, not_op_count));
        }

        match &expr.kind {
            ExprKind::Unary(UnOp::Not, _) => not_op_count += 1,
            // Do not lint anything if this `is_empty` call is in function/method's parameter.
            ExprKind::MethodCall(..) | ExprKind::Call(..) => return None,
            _ => (),
        }
    }
    None
}

/// Populate separated suggestion strings,
/// one for the if condition;
/// one for `then` block and one for `else` block both with unnecessary index expr replaced.
fn make_suggestion(
    cx: &LateContext<'_>,
    if_expr: &IfExprWithIsEmpty<'_>,
    receiver: &Expr<'_>,
) -> Option<(String, String, String)> {
    let mut app = Applicability::MaybeIncorrect;
    let caller = snippet_with_applicability(cx, receiver.span, "_", &mut app);
    let cond_sugg = format!("let x = {caller}.first()");
    // If this `if` expr is previously `if _.is_empty()`,
    // after replacing it to `if let Some(..) = _.first()`,
    // we should switch the order of `then` and `else` block.
    let (then_replacement, else_replacement) = if if_expr.if_is_empty {
        (
            snippet_with_applicability(cx, if_expr.higher_if.r#else?.span, "..", &mut app).to_string(),
            snippet_with_applicability(cx, if_expr.higher_if.then.span, "..", &mut app).to_string(),
        )
    } else {
        (
            snippet_with_applicability(cx, if_expr.higher_if.then.span, "..", &mut app)
                .replace(&format!("{caller}[0]"), "x"),
            snippet_with_applicability(cx, if_expr.higher_if.r#else?.span, "..", &mut app).to_string(),
        )
    };

    Some((cond_sugg, then_replacement, else_replacement))
}
