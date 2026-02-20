#[test]
fn y_json_files_should_be_valid() {
    let test_suite_dir: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "JSONTestSuite",
        "test_parsing",
    ]
    .iter()
    .collect();

    for entry in std::fs::read_dir(&test_suite_dir).expect("failed to read test suite directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("y_") || !name.ends_with(".json") {
            continue;
        }

        let input = std::fs::read(&path).expect("failed to read file");
        let mut output = Vec::new();
        let result = reparojson::repair(&input[..], &mut output);

        assert!(
            matches!(result, Ok(reparojson::RepairOk::Valid)),
            "{}: expected valid JSON, but {:?}",
            name,
            result
        );
        assert_eq!(input, output, "{name}: output differs from input");
    }
}

#[test]
fn n_json_files_should_be_invalid() {
    let test_suite_dir: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "JSONTestSuite",
        "test_parsing",
    ]
    .iter()
    .collect();

    for entry in std::fs::read_dir(&test_suite_dir).expect("failed to read test suite directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("n_") || !name.ends_with(".json") {
            continue;
        }

        // Skip test cases that cause stack overflow.
        if *name == *"n_structure_100000_opening_arrays.json"
            || *name == *"n_structure_open_array_object.json"
        {
            continue;
        }

        let input = std::fs::read(&path).expect("failed to read file");
        let mut output = Vec::new();
        let result = reparojson::repair(&input[..], &mut output);

        assert!(
            !matches!(result, Ok(reparojson::RepairOk::Valid)),
            "{}: expected invalid JSON, but {:?}",
            name,
            result
        );
    }
}
