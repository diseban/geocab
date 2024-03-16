#![cfg_attr(not(feature = "export-abi"), no_std)]
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

/// Use an efficient WASM allocator.
#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

use alloc::{string::String, vec::Vec};
use alloy_sol_types::sol;
/// Import items from the SDK. The prelude contains common traits and macros.
use stylus_sdk::{
    alloy_primitives::{Address, U256},
    evm,
    prelude::*,
    storage::{StorageAddress, StorageI128, StorageMap, StorageString, StorageU256, StorageVec},
};
use substrate_fixed::types::I64F64;
use substrate_geohash::GeoHash;

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
    pub fn publish_driver_locations(&mut self, drivers: Vec<(Address, i128, i128)>) {
        //self.number.set(drivers[0].1);
        for driver in drivers {
            let (address, lat_input, lon_input) = driver;
            let hash = encode_geohash(lat_input, lon_input);
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
    }

    /// Gets numbers at a geohash
    pub fn driver_at_geohash(&self, geohash: String) -> Result<Vec<Address>, Vec<u8>> {
        let drivers = self.driver_at_geohash(geohash).unwrap();
        Ok(drivers)
    }

    /// Books a trip
    pub fn book_trip(
        &self,
        origin: (i128, i128),
        destination: (i128, i128),
    ) -> Result<(), Vec<u8>> {
        let origin_hash = encode_geohash(origin.0, origin.1);
        let nearby_drivers = self.drivers_on_grid.get(origin_hash);
        let driver_location = nearby_drivers.get(0).ok_or_else(Vec::new)?;
        evm::log(TripBooked {
            passenger: Address::ZERO,
            driver: driver_location.address.get(),
        });
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

fn encode_geohash(lat: i128, lon: i128) -> String {
    let lat = I64F64::from_num(lat);
    let lon = I64F64::from_num(lon);
    GeoHash::<9>::try_from_params(lat, lon).unwrap().into()
}

sol! {
    event TripBooked(address indexed passenger, address indexed driver);
}
/*
fn foo() {
    evm::log(TripBooked {
        passenger: Address::ZERO,
        driver: address,
        value,
    });
}
*/
