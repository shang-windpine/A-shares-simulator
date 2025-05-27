//! # 撮合引擎 (Matching Engine)
//! 
//! 本模块实现了A股模拟交易软件的核心撮合引擎，负责订单匹配和交易生成。

pub mod types;
pub mod traits;
pub mod engine;
pub mod errors;

pub use types::*;
pub use traits::*;
pub use engine::*;
pub use errors::*;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
} 