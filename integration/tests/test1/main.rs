#[cfg(test)]
mod test {
    use policies::*;

    #[test]
    fn test_test() {
        let data: Data = Data::new(MetaData::new(3, String::from("laksn"), None), String::from("wdk"), None);

        let res = can_read_data(data);
        println!("{res:?}");

        // assert!(false, "uh oh");
    }
}