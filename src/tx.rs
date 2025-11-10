use crate::util::parse_decimal;
use crate::util::serialize_decimal;
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Type {
    /// A deposit. This is a root transaction.
    Deposit {
        #[serde(
            deserialize_with = "parse_decimal",
            serialize_with = "serialize_decimal"
        )]
        amount: Decimal,
    },

    /// A withdrawal. This is a root transaction.
    Withdrawal {
        #[serde(
            deserialize_with = "parse_decimal",
            serialize_with = "serialize_decimal"
        )]
        amount: Decimal,
    },

    /// A dispute of another transaction. [`Transaction::tx`] refers to the transaction id that is
    /// being disputed.
    ///
    /// This is not a root transaction.
    Dispute,

    /// A resolution of a previously disputed transaction. [`Transaction::tx`] refers to the
    /// transaction id that is being resolved.
    ///
    /// This is not a root transaction.
    Resolve,

    /// A chargeback of a previously disputed transaction. [`Transaction::tx`] refers to the
    /// transaction id that is being charged back.
    ///
    /// This is not a root transaction.
    Chargeback,
}

impl Type {
    /// Whether this is a "root" transaction, i.e. a transaction that does _not_ refer to other
    /// transactions.
    pub const fn is_root(&self) -> bool {
        matches!(
            self,
            Self::Deposit { amount: _ } | Self::Withdrawal { amount: _ }
        )
    }
}

/// The state of a transaction. This is only relevant for "root" transactions, i.e. transactions
/// that don't refer to other transactions.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// A new transaction (just came in) that hasn't been processed yet.
    #[default]
    NeedsProcessing,

    /// Transaction has been processed and funds have been either been credited to debited
    /// successfully.
    Processed,

    /// This transaction is disputed, and the associated funds have been placed on hold.
    Disputed,

    /// This transaction has been charged back by the client.
    ChargedBack,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transaction {
    pub client: u16,
    pub tx: u32,

    #[serde(flatten)]
    pub tx_type: Type,

    #[serde(skip, default)]
    pub state: State,
}

pub trait TransactionBackend {
    /// Create a stream that returns all transactions in chronological order.
    fn create_tx_stream(&self) -> impl StreamExt<Item = Transaction>;

    /// Find a particular transaction.
    fn find_transaction(&self, id: u32) -> impl Future<Output = Option<Transaction>> + Send;

    /// Update the state of a transaction.
    fn set_tx_state(&self, id: u32, state: State) -> impl Future<Output = ()> + Send;
}
