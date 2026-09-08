//! What every peso rail has to be able to say, independent of who is carrying
//! the money.
//!
//! There are two rails now (`paypal`, `stripe`) and they are deliberately not
//! allowed to have their own vocabularies: `infra::payouts` settles a transfer
//! without knowing which provider sent it, and `api::lending` credits a
//! deposit without knowing which one captured it. That is only true because
//! both providers are forced to answer in *these* three shapes.
//!
//! Adding a third rail means implementing these, and nothing else changes.

/// Money in, verified by the provider — never by the client.
pub struct CapturedPayment {
    /// The provider's own reference, already namespaced by rail
    /// (`stripe:pi_…`, or PayPal's bare capture id). This becomes the ledger's
    /// unique `rail_ref`, which is what makes a re-sent confirmation bounce
    /// off the schema instead of crediting twice.
    pub capture_id: String,
    /// Whole centavos actually received.
    pub centavos: i64,
}

/// Where a payout has got to. Only `Paid` is allowed to move the books.
///
/// "Paid" means the same thing on both rails and it is worth being precise
/// about: the money has reached the member's account *with the provider*
/// (their PayPal balance, their Stripe connected balance). Onward settlement
/// to a bank is the provider's business and the member's, not the pool's —
/// the pool's obligation is discharged at this point, which is exactly what
/// posting `payout_payable → cash` records.
pub enum PayoutOutcome {
    /// The recipient has it.
    Paid {
        item_id: String,
        transaction_id: Option<String>,
    },
    /// Accepted, still moving.
    Pending { item_id: Option<String> },
    /// Sent, but the recipient hasn't accepted it. PayPal returns these
    /// automatically after 30 days; Stripe has no equivalent state.
    Unclaimed { item_id: Option<String> },
    /// Came back — refused, returned or reversed. The money is ours again.
    Returned {
        item_id: Option<String>,
        reason: String,
    },
    /// The provider refused it outright.
    Failed { reason: String },
}

/// Submitting a payout can fail in three very different ways, and the caller
/// must not treat them alike.
pub enum SubmitError {
    /// Nothing was sent — safe to retry with the same idempotency key.
    Retryable(String),
    /// The provider refused this payout for good; retrying changes nothing.
    Refused(String),
    /// The provider has already seen this key, so the money may well be on its
    /// way. NEVER retry under a fresh key: look the transfer up by the payout
    /// id it was submitted under and reconcile.
    AlreadySubmitted,
}
