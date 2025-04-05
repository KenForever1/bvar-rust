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

//! 变量的基础定义

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::ptr;

/// 存储所有暴露变量的全局表
static EXPOSED_VARS: Lazy<DashMap<String, VarEntry>> = Lazy::new(|| DashMap::new());

struct VarEntry {
    var_ptr: usize, // 存储变量的指针地址，用于比较身份
    type_id: std::any::TypeId, // 存储类型ID
}

/// 变量基础特性
pub trait Variable: Send + Sync  where Self: 'static{
    /// 将变量描述为字符串
    fn describe(&self, f: &mut String, quote_string: bool) -> bool;
    
    /// 获取变量的描述
    fn get_description(&self) -> String {
        let mut buf = String::new();
        let _ = self.describe(&mut buf, false);
        buf
    }
    
    /// 暴露此变量，使其可以被查询
    fn expose(&self, name: &str) -> i32 {
        self.expose_impl("", name)
    }
    
    /// 使用前缀暴露此变量
    fn expose_as(&self, prefix: &str, name: &str) -> i32 {
        self.expose_impl(prefix, name)
    }
    
    /// 隐藏此变量，使其不能被查询
    fn default_hide(&self) -> bool {
        // 从全局表中移除自己
        let var_name = self.name();
        if var_name.is_empty() {
            return false;
        }
        
        if let Some((_, entry)) = EXPOSED_VARS.remove(&var_name) {
            // 比较指针地址确认是同一个变量
            let self_ptr = ptr::addr_of!(*self) as *const () as usize;
            if entry.var_ptr == self_ptr && 
               entry.type_id == std::any::TypeId::of::<Self>() {
                return true;
            } else {
                // 不是同一个变量，放回去
                let _ = EXPOSED_VARS.insert(var_name, entry);
            }
        }
        
        false
    }

    fn hide(&self) -> bool {
        self.default_hide()
    }
    
    /// 检查变量是否被隐藏
    fn is_hidden(&self) -> bool {
        self.name().is_empty()
    }
    
    /// 获取变量名称
    fn name(&self) -> String {
        String::new()
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32;
    /// 实现暴露变量的方法
    fn default_expose_impl(&self, prefix: &str, name: &str) -> i32 {
        // 构建完整名称
        let full_name = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}_{}", prefix, name)
        };
        
        // 创建变量条目
        let self_ptr = ptr::addr_of!(*self) as *const () as usize;
        let entry = VarEntry {
            var_ptr: self_ptr,
            type_id: std::any::TypeId::of::<Self>(),
        };
        
        if EXPOSED_VARS.contains_key(&full_name) {
            // 名称冲突
            -1
        } else {
            EXPOSED_VARS.insert(full_name, entry);
            0
        }
    }
}

/// 获取暴露变量的数量
pub fn count_exposed() -> usize {
    EXPOSED_VARS.len()
}