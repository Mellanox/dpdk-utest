use regex::{Regex, RegexSet};

enum RepeatPattern {
    Exact = 0,
    Min,
    Max,
    Range,
    Num
}
const REPEAT_PATTERN: [&str; RepeatPattern::Num as usize] = [
    r#"^(\([[:ascii:]]{1,}\))\{(\d{1,})\}$"#,
    r#"^(\([[:ascii:]]{1,}\))\{(\d{1,}),\}$"#,
    r#"^(\([[:ascii:]]{1,}\))\{,(\d{1,})\}$"#,
    r#"^(\([[:ascii:]]{1,}\))\{(\d{1,}),(\d{1,})\}$"#,
];

fn xregex_exact<'a>(h:&'a str) -> Option<(&'a str, usize, usize)> {
    let xregex = Regex::new(REPEAT_PATTERN[RepeatPattern::Exact as usize]).unwrap();

    for (_, [body, count]) in xregex.captures_iter(h).map(|cap| cap.extract()) {
        let num = count.parse::<usize>().unwrap();
        return Some((body, num, num))
    }
    None
}

fn xregex_min<'a>(h:&'a str) -> Option<(&'a str, usize, usize)> {
    let xregex = Regex::new(REPEAT_PATTERN[RepeatPattern::Min as usize]).unwrap();

    for (_, [body, count]) in xregex.captures_iter(h).map(|cap| cap.extract()) {
        let min = count.parse::<usize>().unwrap();
        return Some((body, min, usize::MAX))
    }
    None
}

fn xregex_max<'a>(h:&'a str) -> Option<(&'a str, usize, usize)> {
    let xregex = Regex::new(REPEAT_PATTERN[RepeatPattern::Max as usize]).unwrap();

    for (_, [body, count]) in xregex.captures_iter(h).map(|cap| cap.extract()) {
        let max = count.parse::<usize>().unwrap();
        return Some((body, 1, max))
    }
    None
}

fn xregex_range<'a>(h:&'a str) -> Option<(&'a str, usize, usize)> {
    let xregex = Regex::new(REPEAT_PATTERN[RepeatPattern::Range as usize]).unwrap();

    for (_, [body, left, right]) in xregex.captures_iter(h).map(|cap| cap.extract()) {
        let min = left.parse::<usize>().unwrap();
        let max = right.parse::<usize>().unwrap();
        return if min <= max { Some((body, min, max)) } else { None }
    }
    None
}

type RangeFunc = fn(&str) -> Option<(&str, usize, usize)>;
const RANGE_FUNCS:[RangeFunc; RepeatPattern::Num as usize] = [xregex_exact, xregex_min, xregex_max, xregex_range];

fn xregex_parse<'a>(h:&'a str) -> Option<(&'a str, usize, usize)> {
    let xset = RegexSet::new(REPEAT_PATTERN).unwrap();
    let m = xset.matches(h);
    for i in 0..RepeatPattern::Num as usize {
        if !m.matched(i) { continue }
        return RANGE_FUNCS[i](h)
    };
    None
}


pub fn xregex<'a>(haystack:&str, pattern:&'a str) -> Option<bool> {
    match xregex_parse(pattern) {
        None => return None,
        Some((body, min, max)) => {
            let mut barr = body.as_bytes();
            barr = &barr[1..barr.len() - 1];
            let x = String::from_utf8_lossy(barr);
            let p = format!("(?m)({})",x);
            let re = Regex::new(&p).unwrap();
            if !re.is_match(haystack) { return Some(false) }
            let res = re.find_iter(haystack).count();
            return Some(res >= min && res <= max)
        }
    }
}
