pub fn pad_center(src: String, len: usize) -> String {
    let mut res = String::with_capacity(len);

    res.push_str(&" ".repeat((len - src.len()) / 2));
    res.push_str(&src);
    res.push_str(&" ".repeat(len - res.len()));

    res
}

pub fn pad_center_str(src: &str, len: usize) -> String {
    let mut res = String::with_capacity(len);

    res.push_str(&" ".repeat((len - src.len()) / 2));
    res.push_str(&src);
    res.push_str(&" ".repeat(len - res.len()));

    res
}
