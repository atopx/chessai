use std::sync::OnceLock;

pub struct Book {
    pub data: [[isize; 3]; 12081],
}

static BOOK: OnceLock<Book> = OnceLock::new();

impl Book {
    pub fn get() -> &'static Book {
        BOOK.get_or_init(|| {
            let data = include!("book.dat");
            Book { data }
        })
    }

    // search 二分查找法
    pub fn search(&self, vl: isize) -> Option<usize> {
        let mut low: isize = 0;
        let mut hig: isize = self.data.len() as isize - 1;

        while low <= hig {
            let mid = (low + hig) >> 1;
            let value = self.data[mid as usize][0];
            match value.cmp(&vl) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Equal => return Some(mid as usize),
                std::cmp::Ordering::Greater => hig = mid - 1,
            }
        }
        None
    }
}
