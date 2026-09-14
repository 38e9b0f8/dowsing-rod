fn sum_positive(values: &[i32]) -> i32 {
    let mut total = 0;
    for &value in values { if value > 0 { total += value; } }
    total
}
fn add_positive(items: &[i32]) -> i32 {
    let mut result = 0;
    for &item in items { if item > 0 { result += item; } }
    result
}

struct Calculator;
impl Calculator {
    fn first(&self, value: i32) -> i32 { let result = value * 2; if result > 0 { return result; } 0 }
    fn second(&self, input: i32) -> i32 { let output = input * 2; if output > 0 { return output; } 0 }
}

fn uses_it() { let _increment = |n: i32| n + 1; }
async fn fetch_one() -> i32 { 1 }

trait SkipMe { fn required(&self); }
