use std::collections::HashMap;

use crate::tx::{State, TransactionBackend};
use crate::util::serialize_decimal;
use eyre::Result;
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Account {
    pub client: u16,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

// Need to manually implement `Serialize`, because we are computing `available`, as opposed to
// storing redundant information.
impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct AccountExt {
            client: u16,
            #[serde(serialize_with = "serialize_decimal")]
            available: Decimal,
            #[serde(serialize_with = "serialize_decimal")]
            held: Decimal,
            #[serde(serialize_with = "serialize_decimal")]
            total: Decimal,
            locked: bool,
        }

        AccountExt {
            client: self.client,
            available: self.available(),
            held: self.held,
            total: self.total,
            locked: self.locked,
        }
        .serialize(serializer)
    }
}

impl Account {
    pub fn available(&self) -> Decimal {
        self.total - self.held
    }
}

pub struct Vault<T>
where
    T: TransactionBackend + Sync,
{
    accounts: RwLock<HashMap<u16, Account>>,
    backend: T,
}

impl<T> Vault<T>
where
    T: TransactionBackend + Sync,
{
    pub fn new(backend: T) -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            backend,
        }
    }

    pub async fn get_accounts(&self) -> Vec<Account> {
        self.accounts.read().await.values().cloned().collect()
    }

    pub async fn process_tx_stream(&self) -> Result<()> {
        let stream = self.backend.create_tx_stream();

        stream
            .for_each(async |item| {
                if item.state != State::NeedsProcessing {
                    return;
                }

                let mut accounts = self.accounts.write().await;

                let account = accounts.entry(item.client).or_insert_with(|| Account {
                    client: item.client,
                    held: Decimal::new(0, 4),
                    total: Decimal::new(0, 4),
                    locked: false,
                });

                'a: {
                    match item.tx_type {
                        crate::tx::Type::Deposit { amount } => {
                            account.total += amount;
                        }
                        crate::tx::Type::Withdrawal { amount } => {
                            if account.available() >= amount && !account.locked {
                                account.total -= amount;
                            }
                        }
                        crate::tx::Type::Dispute => {
                            let Some(tx) = self.backend.find_transaction(item.tx).await else {
                                break 'a;
                            };

                            if tx.state != State::Processed {
                                break 'a;
                            }

                            self.backend.set_tx_state(tx.tx, State::Disputed).await;

                            if let crate::tx::Type::Deposit { amount } = &tx.tx_type {
                                account.held += amount;
                            }
                        }
                        crate::tx::Type::Resolve => {
                            let Some(tx) = self.backend.find_transaction(item.tx).await else {
                                break 'a;
                            };

                            if tx.state != State::Disputed {
                                break 'a;
                            }

                            self.backend.set_tx_state(tx.tx, State::Processed).await;

                            if let crate::tx::Type::Deposit { amount } = &tx.tx_type {
                                account.held -= amount;
                            }
                        }
                        crate::tx::Type::Chargeback => {
                            let Some(tx) = self.backend.find_transaction(item.tx).await else {
                                break 'a;
                            };

                            if tx.state != State::Disputed {
                                break 'a;
                            }

                            self.backend.set_tx_state(tx.tx, State::ChargedBack).await;

                            if let crate::tx::Type::Deposit { amount } = &tx.tx_type {
                                account.held -= amount;
                                account.total -= amount;
                                account.locked = true;
                            }
                        }
                    }
                }

                self.backend.set_tx_state(item.tx, State::Processed).await;

                drop(accounts);
            })
            .await;

        Ok(())
    }
}
