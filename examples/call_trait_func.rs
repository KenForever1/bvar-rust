trait AA {
    fn hello(&self) {
        println!("AA 的默认 hello 方法");
    }
}

struct A;

// 实现 Trait AA
impl AA for A {
    fn hello(&self) {
        println!("A 实现的 AA 的 hello");
    }
}


// 不会调用到println!("AA 的默认 hello 方法");
// ‌直接调用默认方法不可行‌：一旦结构体覆盖了 trait 方法，无法通过 <A as AA>::hello(self) 直接访问默认实现


impl A {
    fn call_aa_hello(&self) {
        // 使用完全限定语法调用 Trait 的 hello
        <A as AA>::hello(self);
    }
}


fn main() {
    let a = A;

    a.hello();            // 输出 "A 自己的 hello"
    a.call_aa_hello();    // 输出 "A 实现的 AA 的 hello"
}



// 结构体 A 自身实现同名方法
// impl A {
//     fn hello(&self) {
//         println!("A 自己的 hello");
//     }
// }