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
use variable::Variable;
pub mod detail;

pub mod recorder;
pub mod variable;
pub mod status;
pub mod window;
pub mod reducer;

fn main() {
    println!("Hello, world!");

    // 创建一个整数记录器
    let recorder = recorder::IntRecorder::new();
    println!("is_hidden (初始): {}", recorder.is_hidden());

    // 添加一些样本
    recorder.add(99);
    recorder.add(1);
    recorder.add(99);
    recorder.add(105);

    // 获取当前值和平均值
    let v = recorder.get_value();
    println!("v: {}", v);

    let avg = recorder.average();
    println!("avg: {}", avg);

    // 暴露变量
    let res = recorder.expose("test_recorder");
    println!("expose结果: {}", res);
    println!("is_hidden (暴露后): {}", recorder.is_hidden());
    println!("变量名称: {}", recorder.name());
    
    // 创建另一个记录器，使用前缀
    let recorder2 = recorder::IntRecorder::with_prefix_name("stats", "second_recorder");
    recorder2.add(10);
    recorder2.add(20);
    println!("recorder2名称: {}", recorder2.name());
    println!("recorder2平均值: {}", recorder2.average());
    
    // 查看暴露的变量数量
    let count = variable::count_exposed();
    println!("已暴露的变量数量: {}", count);
    
    // 隐藏变量
    let hide_result = recorder.hide();
    println!("hide结果: {}", hide_result);
    println!("隐藏后的变量数量: {}", variable::count_exposed());
}
