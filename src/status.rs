// Copyright 2025 KenForever1
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! 实现运行时可修改的状态变量

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;
use std::fmt::Write;
use crate::variable::Variable;
use std::cell::UnsafeCell;

/// 表示可变的状态
pub struct Status<T> {
    /// 内部值
    value: RwLock<T>,
    /// 变量名称
    name: UnsafeCell<String>,
    /// 是否已经被暴露
    exposed: AtomicBool,
}

// 手动实现线程安全 - 我们确保对UnsafeCell的访问是安全的
unsafe impl<T> Send for Status<T> {}
unsafe impl<T> Sync for Status<T> {}

impl<T: Clone + fmt::Display + Send + Sync + 'static> Status<T> {
    /// 创建新的状态变量
    pub fn new(value: T) -> Self {
        Self {
            value: RwLock::new(value),
            name: UnsafeCell::new(String::new()),
            exposed: AtomicBool::new(false),
        }
    }
    
    /// 用名称创建
    pub fn with_name(value: T, name: &str) -> Self {
        let status = Self::new(value);
        let _ = status.expose(name);
        status
    }
    
    /// 用前缀和名称创建
    pub fn with_prefix_name(value: T, prefix: &str, name: &str) -> Self {
        let status = Self::new(value);
        let _ = status.expose_as(prefix, name);
        status
    }
    
    /// 获取当前值
    pub fn get_value(&self) -> T {
        self.value.read().clone()
    }
    
    /// 设置新值
    pub fn set_value(&self, value: T) {
        *self.value.write() = value;
    }
}

impl<T: Clone + fmt::Display + Send + Sync + 'static> Variable for Status<T> {
    fn describe(&self, f: &mut String, quote_string: bool) -> bool {
        let value = self.value.read();
        if quote_string && std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
           let _ = write!(f, "\"{}\"", value);
        } else {
            let _ = write!(f, "{}", value);
        }
        true
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32 {
        // 更新内部名称
        let mut full_name = String::new();
        if !prefix.is_empty() {
            full_name.push_str(prefix);
            full_name.push('_');
        }
        full_name.push_str(name);
        
        // 将自己暴露出去
        let result = <Status<T> as Variable>::default_expose_impl(self, prefix, name);
        if result == 0 {
            // 仅在成功时更新名称
            self.exposed.store(true, Ordering::Relaxed);
            // 使用UnsafeCell安全地更新内部状态
            unsafe {
                *self.name.get() = full_name;
            }
        }
        result
    }
    
    fn name(&self) -> String {
        unsafe { (*self.name.get()).clone() }
    }
    
    fn hide(&self) -> bool {
        let result = Variable::default_hide(self);
        if result {
            self.exposed.store(false, Ordering::Relaxed);
        }
        result
    }
    
    fn is_hidden(&self) -> bool {
        !self.exposed.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_status() {
        let status = Status::new(1);
        let _ = status.expose("test");
        let value = status.get_value();
        assert_eq!(value, 1);
        let _ = status.hide();
        let value = status.get_value();
        assert_eq!(value, 1);

        // change value
        status.set_value(2);
        let value = status.get_value();
        assert_eq!(value, 2);
    }
}