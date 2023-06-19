use std::env;
use hdrs::Client;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    let name_node = &args[1];
    let user = &args[2];
    let ticket_path = &args[3];
    let checked_file_path = &args[4];

    let hdfs_client = Client::connect(name_node, user, ticket_path)?;
    let metadata = hdfs_client.metadata(checked_file_path)?;
    println!("file len: {}", metadata.len());

    Ok(())
}