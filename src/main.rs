use lazy_static::lazy_static;
use rand::Rng;
use starknet::core::{crypto::pedersen_hash, types::FieldElement};
use std::fmt::Write;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn to_hex(felt: &FieldElement) -> String {
    let bytes = felt.to_bytes_be();
    let mut result = String::with_capacity(bytes.len() * 2 + 2);
    result.push_str("0x");
    for byte in bytes {
        write!(&mut result, "{:02x}", byte).unwrap();
    }
    result
}

lazy_static! {
    static ref CONST_STRING: FieldElement =
        FieldElement::from_hex_be("0x535441524b4e45545f434f4e54524143545f41444452455353").unwrap();
    static ref DEPLOYER_ADDR: FieldElement = FieldElement::from_hex_be("0x0").unwrap();
    static ref CLASS_HASH: FieldElement = FieldElement::from_hex_be(
        "0x59d886a22f84091b75918faecebfc0d93128000d4b045f57b71d51871453d6f",
    ).unwrap();
    static ref CONSTRUCTOR_PARAMS_HASH: FieldElement = pedersen_hash(&FieldElement::ZERO, &FieldElement::ZERO);
    static ref INITIAL_MIN: FieldElement = FieldElement::from_hex_be(
        "0x800000000000011000000000000000000000000000000000000000000000000",
    ).unwrap(); // P-1

    static ref PRE_COMPUTE : FieldElement = pedersen_hash(
        &pedersen_hash(&FieldElement::ZERO, &CONST_STRING),
        &DEPLOYER_ADDR,
    );
    static ref LEN  : FieldElement = FieldElement::from_str("5").unwrap();
}

fn derive_addr(i: u128) -> FieldElement {
    pedersen_hash(
        &pedersen_hash(
            &pedersen_hash(
                &pedersen_hash(&PRE_COMPUTE, &FieldElement::from(i)),
                &CLASS_HASH,
            ),
            &CONSTRUCTOR_PARAMS_HASH,
        ),
        &LEN,
    )
}

fn find_min(output: mpsc::Sender<(u128, FieldElement)>) {
    let mut min = INITIAL_MIN.clone();
    let mut rng = rand::thread_rng();
    loop {
        let i: u128 = rng.gen();
        let output_addr = derive_addr(i);
        if output_addr < min {
            min = output_addr;
            output.send((i, min.clone())).unwrap();
        }
    }
}

fn estimate_addresses_per_second(num_threads: usize) -> usize {
    let duration = Duration::from_secs(1);
    let (tx, rx) = mpsc::channel();

    for _ in 0..num_threads {
        let tx_clone = tx.clone();
        thread::spawn(move || {
            let start_time = Instant::now();
            let mut count = 0;
            while start_time.elapsed() < duration {
                let _: u128 = rand::thread_rng().gen();
                derive_addr(count);
                count += 1;
            }
            tx_clone.send(count).unwrap();
        });
    }

    drop(tx);

    let results: Vec<u128> = rx.iter().collect();
    let total: usize = results.iter().map(|&val| val as usize).sum();
    total
}

fn main() {
    let global_min = Arc::new(Mutex::new(INITIAL_MIN.clone()));

    let (tx, rx) = mpsc::channel();

    let available_threads = num_cpus::get();
    let num_threads = if available_threads > 1 {
        available_threads
    } else {
        1
    };
    println!("will use {} threads", num_threads);

    let estimated_addresses = estimate_addresses_per_second(num_threads);
    println!(
        "Estimated speed: {:.1}k/s",
        estimated_addresses as f64 / 1000.0
    );

    for _ in 0..num_threads {
        let tx_clone = tx.clone();
        thread::spawn(move || find_min(tx_clone));
    }

    drop(tx);

    for (i, min) in rx {
        let mut global_min_lock = global_min.lock().unwrap();
        if min < *global_min_lock {
            *global_min_lock = min.clone();
            println!("salt {}, min: {}", i, to_hex(&min));
        }
    }
}
