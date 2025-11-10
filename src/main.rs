use std::{env, io};

use ::csv::Writer;

use crate::{account::Vault, csv::CsvBackend};

mod account;
mod csv;
mod tx;
mod util;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let filename = args.get(1).expect("no file name was provided");

    let tx_backend = CsvBackend::new(filename.to_owned());

    let vault = Vault::new(tx_backend);

    vault.process_tx_stream().await;

    let accounts = vault.get_accounts().await;

    let mut writer = Writer::from_writer(io::stdout());

    for account in accounts {
        writer
            .serialize(account)
            .expect("failed to serialize account");
    }
}
