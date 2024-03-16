//!
//! Stylus Hello World
//!
//! The following contract implements the Geocab example from Foundry.
//!
//! ```
//! contract Geocab {
//!     uint256 public number;
//!     function setNumber(uint256 newNumber) public {
//!         number = newNumber;
//!     }
//!     function increment() public {
//!         number++;
//!     }
//! }
//! ```
//!
//! The program is ABI-equivalent with Solidity, which means you can call it from both Solidity and Rust.
//! To do this, run `cargo stylus export-abi`.
//!
//! Note: this code is a template-only and has not been audited.
//!

// Allow `cargo stylus export-abi` to generate a main function.
#![no_std]
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

/// Use an efficient WASM allocator.
#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

use alloc::{string::String, vec::Vec};
/// Import items from the SDK. The prelude contains common traits and macros.
use stylus_sdk::{
    alloy_primitives::{Address, I128, U256},
    contract::address,
    prelude::*,
    storage::{StorageAddress, StorageI128, StorageMap, StorageString, StorageU256, StorageVec},
};
use substrate_fixed::types::I64F64;
use substrate_geohash::GeoHash;

// Define some persistent storage using the Solidity ABI.
// `Geocab` will be the entrypoint.
// sol_storage! {
//     #[entrypoint]
//     pub struct Geocab {
//         uint256 number;
//     }
// }

#[entrypoint]
#[solidity_storage]
pub struct Geocab {
    number: StorageU256,
    drivers_on_grid: StorageMap<String, StorageVec<DriverLocation>>,
    driver_grid: StorageMap<Address, StorageString>,
}

#[solidity_storage]
pub struct DriverLocation {
    pub address: StorageAddress,
    pub lat: StorageI128,
    pub lon: StorageI128,
}

/// Declare that `Geocab` is a contract with the following external methods.
#[external]
impl Geocab {
    /// Publish driver locations
    pub fn publish_driver_locations(
        &mut self,
        drivers: Vec<(Address, i128, i128)>,
    ) -> Result<(), Vec<u8>> {
        //self.number.set(drivers[0].1);
        for driver in drivers {
            let (address, lat_input, lon_input) = driver;
            let lat = I64F64::from_num(lat_input);
            let lon = I64F64::from_num(lon_input);
            let hash: String = GeoHash::<9>::try_from_params(lat, lon).unwrap().into();
            // mapping driver address -> geohash
            let mut guard = self.driver_grid.setter(address);
            guard.set_str(&hash);
            // mapping geohash -> driver
            let mut guard = self.drivers_on_grid.setter(hash);
            let mut driver = guard.grow();
            driver.address.set(address);
            driver.lat.set(lat_input.try_into().unwrap());
            driver.lon.set(lon_input.try_into().unwrap());
        }
        Ok(())
    }
    /// Gets the number from storage.
    pub fn number(&self) -> Result<U256, Vec<u8>> {
        Ok(U256::from(self.number.get()))
    }

    /// Sets a number in storage to a user-specified value.
    pub fn set_number(&mut self, new_number: U256) {
        self.number.set(new_number);
    }

    /// Increments `number` and updates its value in storage.
    pub fn increment(&mut self) {
        let number = self.number.get();
        self.set_number(number + U256::from(1));
    }
}
