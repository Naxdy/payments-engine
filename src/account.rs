use std::collections::HashMap;

use crate::tx::{State, TransactionBackend};
use crate::util::serialize_decimal;
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

    pub async fn process_tx_stream(&self) {
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

                            self.backend.set_tx_state(item.tx, State::Processed).await;
                        }
                        crate::tx::Type::Withdrawal { amount } => {
                            if account.available() >= amount && !account.locked {
                                account.total -= amount;
                            }

                            self.backend.set_tx_state(item.tx, State::Processed).await;
                        }
                        crate::tx::Type::Dispute => {
                            let Some(tx) = self.backend.find_transaction(item.tx).await else {
                                break 'a;
                            };

                            if tx.client != account.client || tx.state != State::Processed {
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

                            if tx.client != account.client || tx.state != State::Disputed {
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

                            if tx.client != account.client || tx.state != State::Disputed {
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

                drop(accounts);
            })
            .await;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        account::{Account, Vault},
        tx::{State, Transaction, Type},
    };

    use std::collections::HashMap;

    use futures::stream;
    use tokio::sync::RwLock;

    use crate::tx::TransactionBackend;

    pub struct MemoryBackend {
        txns: Vec<Transaction>,
        tx_states: RwLock<HashMap<u32, State>>,
    }

    impl MemoryBackend {
        pub fn new(txns: Vec<Transaction>) -> Self {
            Self {
                txns,
                tx_states: RwLock::new(HashMap::new()),
            }
        }
    }

    impl TransactionBackend for MemoryBackend {
        fn create_tx_stream(&self) -> impl futures::StreamExt<Item = Transaction> {
            stream::iter(self.txns.iter().cloned())
        }

        async fn find_transaction(&self, id: u32) -> Option<Transaction> {
            let tx_states = self.tx_states.read().await;

            self.txns.iter().cloned().find_map(|mut e| {
                if e.tx == id && e.tx_type.is_root() {
                    e.state = tx_states
                        .get(&e.tx)
                        .map_or(State::NeedsProcessing, |state| *state);

                    Some(e)
                } else {
                    None
                }
            })
        }

        async fn set_tx_state(&self, id: u32, state: super::State) {
            self.tx_states.write().await.insert(id, state);
        }
    }

    async fn make_accounts_from_txns(txns: Vec<Transaction>) -> Vec<Account> {
        let backend = MemoryBackend::new(txns);

        let vault = Vault::new(backend);

        vault.process_tx_stream().await;

        vault.get_accounts().await
    }

    #[tokio::test]
    async fn dispute() {
        let txns = [
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Deposit { amount: 10.into() },
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Dispute,
            },
        ];

        let accounts = make_accounts_from_txns(txns.to_vec()).await;

        assert!(accounts[0].total > 0.into());
        assert_eq!(accounts[0].held, accounts[0].total);
        assert_eq!(accounts[0].available(), 0.into());
    }

    #[tokio::test]
    async fn dispute_resolution() {
        let txns = [
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Deposit { amount: 10.into() },
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Dispute,
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Resolve,
            },
        ];

        let accounts = make_accounts_from_txns(txns.to_vec()).await;

        assert!(accounts[0].total > 0.into());
        assert_eq!(accounts[0].held, 0.into());
        assert_eq!(accounts[0].total, accounts[0].available());
    }

    #[tokio::test]
    async fn close() {
        let txns = [
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Deposit { amount: 10.into() },
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Dispute,
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Chargeback,
            },
        ];

        let accounts = make_accounts_from_txns(txns.to_vec()).await;

        assert!(accounts[0].locked);
        assert_eq!(accounts[0].total, 0.into());
        assert_eq!(accounts[0].held, accounts[0].total);
        assert_eq!(accounts[0].available(), 0.into());
    }

    #[tokio::test]
    async fn withdrawal() {
        let txns = [
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Deposit { amount: 10.into() },
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Withdrawal { amount: 5.into() },
            },
        ];

        let accounts = make_accounts_from_txns(txns.to_vec()).await;

        assert_eq!(accounts[0].total, 5.into());
        assert_eq!(accounts[0].total, accounts[0].available());
    }

    #[tokio::test]
    async fn withdrawal_fail() {
        let txns = [
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Deposit { amount: 10.into() },
            },
            Transaction {
                client: 1,
                tx: 1,
                state: State::default(),
                tx_type: Type::Withdrawal { amount: 15.into() },
            },
        ];

        let accounts = make_accounts_from_txns(txns.to_vec()).await;

        assert_eq!(accounts[0].total, 10.into());
        assert_eq!(accounts[0].total, accounts[0].available());
    }
}
