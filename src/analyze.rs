use std::path::PathBuf;

use glob::glob;
use polars::prelude::*;

pub fn analyze_dump(input: PathBuf, _output: PathBuf) -> color_eyre::Result<()> {
    // Define the pattern to find all CSV files with "acc", "mag", or "gyro" in their names
    let pattern = input.join("*.csv");

    // Iterate over each file that matches the pattern
    for entry in glob(&format!("{}", pattern.display())).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                // Check if the file name contains "acc", "mag", or "gyro"
                if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                    if file_name.contains("acc")
                        || file_name.contains("mag")
                        || file_name.contains("gyro")
                    {
                        // Read the CSV file using Polars
                        let some = CsvReadOptions::default()
                            .with_has_header(true)
                            .try_into_reader_with_file_path(Some(path))?
                            .finish()?;

                        println!("{:?}", some.get_column_names());
                    }
                }
            }
            Err(e) => eprintln!("Failed to read path: {:?}", e),
        }
    }
    Ok(())
}
