/// SIMD 批量撮合优化工具
use wide::*;

/// SIMD 批量价格比较（i64x4）
#[inline]
pub fn simd_price_compare_le(prices: &[i64], limit: i64) -> Vec<bool> {
    let mut result = Vec::with_capacity(prices.len());
    
    let chunks = prices.chunks_exact(4);
    let remainder = chunks.remainder();
    
    // 批量处理（4 个一组，展开循环提升性能）
    for chunk in chunks {
        result.push(chunk[0] <= limit);
        result.push(chunk[1] <= limit);
        result.push(chunk[2] <= limit);
        result.push(chunk[3] <= limit);
    }
    
    // 处理剩余元素
    for &price in remainder {
        result.push(price <= limit);
    }
    
    result
}

/// SIMD 批量价格比较（i64x4，大于等于）
#[inline]
pub fn simd_price_compare_ge(prices: &[i64], limit: i64) -> Vec<bool> {
    let mut result = Vec::with_capacity(prices.len());
    
    let chunks = prices.chunks_exact(4);
    let remainder = chunks.remainder();
    
    for chunk in chunks {
        result.push(chunk[0] >= limit);
        result.push(chunk[1] >= limit);
        result.push(chunk[2] >= limit);
        result.push(chunk[3] >= limit);
    }
    
    for &price in remainder {
        result.push(price >= limit);
    }
    
    result
}

/// SIMD 批量数量累加（i64x4）
#[inline]
pub fn simd_sum_sizes(sizes: &[i64]) -> i64 {
    let mut sum = i64x4::splat(0);
    
    let chunks = sizes.chunks_exact(4);
    let remainder = chunks.remainder();
    
    for chunk in chunks {
        let size_vec = i64x4::new([chunk[0], chunk[1], chunk[2], chunk[3]]);
        sum = sum + size_vec;
    }
    
    let arr = sum.to_array();
    let mut total = arr[0] + arr[1] + arr[2] + arr[3];
    
    for &size in remainder {
        total += size;
    }
    
    total
}

/// SIMD 批量最小值计算（i64x4）
#[inline]
pub fn simd_min_pairs(a: &[i64], b: &[i64]) -> Vec<i64> {
    assert_eq!(a.len(), b.len());
    let mut result = Vec::with_capacity(a.len());
    
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();
    
    for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
        // 手动最小值计算
        result.push(chunk_a[0].min(chunk_b[0]));
        result.push(chunk_a[1].min(chunk_b[1]));
        result.push(chunk_a[2].min(chunk_b[2]));
        result.push(chunk_a[3].min(chunk_b[3]));
    }
    
    for (a, b) in remainder_a.iter().zip(remainder_b.iter()) {
        result.push((*a).min(*b));
    }
    
    result
}

/// SIMD 批量相减（i64x4）
#[inline]
pub fn simd_sub_vectors(a: &[i64], b: &[i64]) -> Vec<i64> {
    assert_eq!(a.len(), b.len());
    let mut result = Vec::with_capacity(a.len());
    
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();
    
    for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
        let vec_a = i64x4::new([chunk_a[0], chunk_a[1], chunk_a[2], chunk_a[3]]);
        let vec_b = i64x4::new([chunk_b[0], chunk_b[1], chunk_b[2], chunk_b[3]]);
        let diff = vec_a - vec_b;
        
        let arr = diff.to_array();
        result.push(arr[0]);
        result.push(arr[1]);
        result.push(arr[2]);
        result.push(arr[3]);
    }
    
    for (a, b) in remainder_a.iter().zip(remainder_b.iter()) {
        result.push(a - b);
    }
    
    result
}

/// 批量订单匹配预处理（SIMD 加速）
#[inline]
pub fn simd_batch_match_prepare(
    sizes: &[i64],
    filled: &[i64],
    need_size: i64,
) -> (Vec<i64>, i64) {
    // 计算每个订单的剩余量
    let remaining = simd_sub_vectors(sizes, filled);
    
    // 累加可用量
    let mut available = 0i64;
    let mut matched_sizes = Vec::with_capacity(remaining.len());
    
    for &rem in &remaining {
        if available >= need_size {
            matched_sizes.push(0);
        } else {
            let can_match = rem.min(need_size - available);
            matched_sizes.push(can_match);
            available += can_match;
        }
    }
    
    (matched_sizes, available)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simd_price_compare() {
        let prices = vec![100, 200, 300, 400, 500, 600];
        let result = simd_price_compare_le(&prices, 350);
        assert_eq!(result, vec![true, true, true, false, false, false]);
    }
    
    #[test]
    fn test_simd_sum() {
        let sizes = vec![10, 20, 30, 40, 50];
        let sum = simd_sum_sizes(&sizes);
        assert_eq!(sum, 150);
    }
    
    #[test]
    fn test_simd_min_pairs() {
        let a = vec![10, 20, 30, 40];
        let b = vec![15, 10, 35, 30];
        let result = simd_min_pairs(&a, &b);
        assert_eq!(result, vec![10, 10, 30, 30]);
    }
}

