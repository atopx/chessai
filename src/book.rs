use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::str::FromStr;
use std::sync::OnceLock;

pub struct Book {
    pub data: Vec<[isize; 3]>,
}

static BOOK: OnceLock<Book> = OnceLock::new();

impl Book {
    pub fn get() -> &'static Book {
        BOOK.get_or_init(|| {
            let mut reader = BufReader::new(File::open("book.dat").unwrap());
            Book { data: bincode::deserialize_from(&mut reader).unwrap() }
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

/// input file `book.txt` sample:
/// 203040,34229,6
/// 509427,33955,1
/// 1435796,50371,2
pub fn builder(input_file: &str, output_file: Option<String>) {
    let input = File::open(input_file).unwrap();
    let mut book: Vec<[isize; 3]> = Vec::new();
    let buffered = BufReader::new(input);
    for line in buffered.lines() {
        let mut record: Vec<isize> = Vec::new();
        for i in line.unwrap().split(",").collect::<Vec<&str>>() {
            record.push(FromStr::from_str(i).unwrap());
        }
        if record.len() == 3 {
            book.push([record[0], record[1], record[2]])
        }
    }
    let mut writer = match output_file {
        Some(out) => BufWriter::new(File::create(out).unwrap()),
        None => BufWriter::new(File::create("book.dat").unwrap()),
    };
    bincode::serialize_into(&mut writer, &book).unwrap();
    println!("success, write {} pieces of `book.dat`", book.len());
}
