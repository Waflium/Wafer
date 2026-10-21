use std::cmp::Ordering;

pub struct TopKSelector<T, C> {
    k: usize,
    comparator: C,
    buffer: Vec<T>,
    threshold: Option<T>,
}

impl<T: Copy, C: Fn(&T, &T) -> Ordering> TopKSelector<T, C> {
    pub fn least(k: usize, comparator: C) -> TopKSelector<T, C> {
        TopKSelector { k, comparator, buffer: Vec::with_capacity(k * 2), threshold: None }
    }

    pub fn offer(&mut self, elem: T) {
        if self.k == 0 {
            return;
        }
        let Some(threshold) = self.threshold else {
            self.buffer.push(elem);
            self.threshold = Some(elem);
            return;
        };
        if self.buffer.len() < self.k {
            self.buffer.push(elem);
            if (self.comparator)(&elem, &threshold) == Ordering::Greater {
                self.threshold = Some(elem);
            }
        } else if (self.comparator)(&elem, &threshold) == Ordering::Less {
            self.buffer.push(elem);
            if self.buffer.len() == 2 * self.k {
                self.trim();
            }
        }
    }

    fn trim(&mut self) {
        let mut left = 0;
        let mut right = 2 * self.k - 1;
        let mut min_threshold_position = 0;
        let mut iterations = 0;
        let max_iterations = ceil_log2(right - left) * 3;
        while left < right {
            let pivot_index = (left + right + 1) >> 1;
            let pivot_new_index = self.partition(left, right, pivot_index);
            if pivot_new_index > self.k {
                right = pivot_new_index - 1;
            } else if pivot_new_index < self.k {
                left = pivot_new_index.max(left + 1);
                min_threshold_position = pivot_new_index;
            } else {
                break;
            }
            iterations += 1;
            if iterations >= max_iterations {
                let comparator = &self.comparator;
                self.buffer[left..=right].sort_by(comparator);
                break;
            }
        }
        self.buffer.truncate(self.k);
        let mut threshold = self.buffer[min_threshold_position];
        for &elem in &self.buffer[min_threshold_position + 1..self.k] {
            if (self.comparator)(&elem, &threshold) == Ordering::Greater {
                threshold = elem;
            }
        }
        self.threshold = Some(threshold);
    }

    fn partition(&mut self, left: usize, right: usize, pivot_index: usize) -> usize {
        let pivot_value = self.buffer[pivot_index];
        self.buffer[pivot_index] = self.buffer[right];
        let mut pivot_new_index = left;
        for i in left..right {
            if (self.comparator)(&self.buffer[i], &pivot_value) == Ordering::Less {
                self.buffer.swap(pivot_new_index, i);
                pivot_new_index += 1;
            }
        }
        self.buffer[right] = self.buffer[pivot_new_index];
        self.buffer[pivot_new_index] = pivot_value;
        pivot_new_index
    }

    pub fn top_k(mut self) -> Vec<T> {
        let comparator = &self.comparator;
        self.buffer.sort_by(comparator);
        self.buffer.truncate(self.k);
        self.buffer
    }
}

fn ceil_log2(x: usize) -> usize {
    (usize::BITS - (x - 1).leading_zeros()) as usize
}

pub fn least<T: Copy>(k: usize, elements: impl IntoIterator<Item = T>, comparator: impl Fn(&T, &T) -> Ordering) -> Vec<T> {
    let mut selector = TopKSelector::least(k, comparator);
    for elem in elements {
        selector.offer(elem);
    }
    selector.top_k()
}

#[cfg(test)]
mod tests {
    fn least_of(keys: &[i32], k: usize) -> Vec<usize> {
        super::least(k, 0..keys.len(), |a, b| keys[*a].cmp(&keys[*b]))
    }

    #[test]
    fn picks_ties_like_guava() {
        let keys = [1, 1, 0, 0, 3, 0, 3, 1, 3, 3, 3, 2, 3, 1, 0, 4, 2, 0, 0, 1, 0, 0, 4, 2, 3, 0, 4, 4, 4, 3, 2, 1, 4, 2, 2, 3];
        assert_eq!(least_of(&keys, 9), [21, 2, 3, 5, 20, 14, 17, 18, 25]);
        let keys = [3, 1, 7, 0, 0, 3, 7, 1, 1, 4, 2, 9, 2, 4, 9, 8, 9, 4, 3, 1, 3, 9, 8, 9, 3, 4, 0, 6, 5, 6, 3];
        assert_eq!(least_of(&keys, 5), [3, 4, 26, 1, 8]);
        let keys = [9, 8, 3, 6, 5, 9, 4, 0, 9, 2, 3, 1, 6, 0, 5, 1, 6, 2, 8, 8, 4, 7, 7, 1, 0, 3, 0, 6, 9, 2, 5, 9, 9, 0];
        assert_eq!(least_of(&keys, 13), [13, 7, 24, 26, 33, 11, 15, 23, 9, 17, 29, 25, 2]);
    }
}
