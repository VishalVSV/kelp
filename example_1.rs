fn main() -> Result<(), Box<dyn Error>> {
    let mut i = 0;

    let long_string = "
                        multi line string highlighting!
                      ";

    while i <= 10 {
        i += 1;
        println!("{}",i);
        // Comments??
        /*
            Multi line syntax highlighting too
        */
    }
}