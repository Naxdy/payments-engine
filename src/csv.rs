use std::collections::HashMap;

use csv::ReaderBuilder;
use futures::stream::{self};
use tokio::sync::RwLock;

use crate::tx::{State, Transaction, TransactionBackend};

// NOTE: With this implementation, tx states are accumulated in memory for every "root"
// transaction, although individual transactions are only kept in memory for as long as they are
// needed.
pub struct CsvBackend {
    filepath: String,
    tx_states: RwLock<HashMap<u32, State>>,
}

impl CsvBackend {
    pub fn new(filepath: String) -> Self {
        Self {
            filepath,
            tx_states: RwLock::new(HashMap::new()),
        }
    }
}

impl TransactionBackend for CsvBackend {
    fn create_tx_stream(&self) -> impl futures::StreamExt<Item = crate::tx::Transaction> {
        let reader = ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(self.filepath.clone())
            .expect("failed to create csv reader");

        stream::iter(
            reader
                .into_deserialize()
                .map(|e| e.expect("failed to parse csv line")),
        )
    }

    async fn find_transaction(&self, id: u32) -> Option<crate::tx::Transaction> {
        let mut reader = ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(self.filepath.clone())
            .expect("failed to create csv reader");

        let tx_states = self.tx_states.read().await;

        reader.deserialize().find_map(|e: Result<Transaction, _>| {
            if let Ok(mut e) = e
                && e.tx == id
                && e.tx_type.is_root()
            {
                e.state = tx_states
                    .get(&e.tx)
                    .map_or(State::NeedsProcessing, |state| *state);

                Some(e)
            } else {
                None
            }
        })
    }

    async fn set_tx_state(&self, id: u32, state: crate::tx::State) {
        self.tx_states.write().await.insert(id, state);
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod test {
    use rust_decimal::Decimal;

    use crate::{account::Vault, csv::CsvBackend};

    #[tokio::test]
    async fn csv_txns() {
        let backend = CsvBackend::new(String::from("testdata/transactions.csv"));

        let vault = Vault::new(backend);

        vault.process_tx_stream().await;

        let accounts = vault.get_accounts().await;

        assert_eq!(
            accounts.iter().find(|e| e.client == 1).unwrap().total,
            Decimal::new(3, 0)
        );
        assert_eq!(
            accounts.iter().find(|e| e.client == 2).unwrap().total,
            Decimal::new(2, 0)
        );
    }
}
