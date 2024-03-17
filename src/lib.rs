#![cfg_attr(not(feature = "export-abi"), no_std)]
#![cfg_attr(not(any(feature = "export-abi", test)), no_main)]
extern crate alloc;

/// Use an efficient WASM allocator.
#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

const OWNER: Address = Address::new([
    0x80, 0x31, 0x0f, 0xA9, 0xcE, 0x4C, 0x31, 0x80, 0x38, 0x12, 0x1C, 0x10, 0x71, 0x62, 0xb8, 0x8F,
    0x1E, 0xC1, 0x4A, 0xF6,
]);

use core::ops::Deref;

use alloc::{string::String, vec::Vec};
use alloy_primitives::Signed;
use alloy_sol_types::sol;
use stylus_sdk::{
    alloy_primitives::{Address, U256},
    call::transfer_eth,
    evm, msg,
    prelude::*,
    storage::{StorageAddress, StorageI128, StorageMap, StorageString, StorageU256, StorageVec},
};
use substrate_fixed::types::I64F64;
use substrate_geohash::GeoHash;

#[entrypoint]
#[solidity_storage]
pub struct Geocab {
    number: StorageU256,
    drivers_on_grid: StorageMap<String, StorageVec<DriverLocationStorage>>,
    driver_grid: StorageMap<Address, StorageString>,
    active_trips: StorageMap<Address, Trip>,
    per_trip_fee: StorageU256,
}

#[solidity_storage]
pub struct DriverLocationStorage {
    pub address: StorageAddress,
    pub lat: StorageI128,
    pub lon: StorageI128,
}

impl DriverLocationStorage {
    fn location(&self) -> Location {
        let lat = to_fixed_signed(self.lat.get());
        let lon = to_fixed_signed(self.lon.get());
        Location { lat, lon }
    }
}

#[solidity_storage]
#[derive(Erase)]
pub struct Trip {
    pub passenger: StorageAddress,
    pub driver: StorageAddress,
    pub value: StorageU256,
}

#[external]
impl Geocab {
    /// Publish driver locations
    pub fn publish_driver_locations(&mut self, drivers: Vec<(Address, i128, i128)>) {
        let num_drivers = drivers.len();
        for driver in drivers {
            let (address, lat_input, lon_input) = driver;
            let hash = encode_geohash(&Location::from_i128_tuple((lat_input, lon_input))).into();
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
        self.number.set(self.number.get() + U256::from(num_drivers))
    }

    /// Gets drivers at a geohash
    pub fn drivers_at_geohash(&self, geohash: String) -> Result<Vec<Address>, Vec<u8>> {
        let mut result = Vec::new();
        let drivers = self.drivers_on_grid.get(geohash);
        for i in 0..drivers.len() {
            result.push(drivers.get(i).unwrap().address.get())
        }
        Ok(result)
    }

    /// Books a trip
    pub fn book_trip(&mut self, origin: (i128, i128), destination: (i128, i128)) {
        let passenger_address = msg::sender();
        let payment = msg::value();
        let origin_location = Location::from_i128_tuple(origin);
        let nearby_drivers = self.all_nearby_drivers(&origin_location);
        let closest_driver = closest_driver(&nearby_drivers, &origin_location);
        let mut new_trip = self.active_trips.setter(passenger_address);
        new_trip.driver.set(closest_driver.address);
        new_trip.passenger.set(passenger_address);
        new_trip.value.set(payment);
        evm::log(TripBooked {
            passenger: passenger_address,
            driver: closest_driver.address,
            dest_lat: destination.0,
            dest_lon: destination.1,
        });
    }

    pub fn complete_trip(&mut self, success: bool) -> Result<(), Vec<u8>> {
        let trip = self.active_trips.get(msg::sender());
        if success {
            let trip_value = trip.value.get();
            let driver_payment = trip_value - self.per_trip_fee.get();
            transfer_eth(trip.driver.get(), driver_payment)?;
            transfer_eth(OWNER, self.per_trip_fee.get())?;
        }
        Ok(())
    }

    pub fn set_fee(&mut self, new_per_trip_fee: U256) {
        if msg::sender() != OWNER {
            panic!("Not owner")
        }
        self.per_trip_fee.set(new_per_trip_fee);
    }

    pub fn active_trip_driver(&self) -> Result<Address, Vec<u8>> {
        let result = self.active_trips.get(msg::sender());
        Ok(result.driver.get())
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

impl Geocab {
    fn all_nearby_drivers(&self, location: &Location) -> Vec<Driver> {
        let position_hash = encode_geohash(location);
        let mut all_position_hashes = all_neighbors(&position_hash);
        let mut result = Vec::new();
        all_position_hashes.push(position_hash);
        for position_hash in all_position_hashes {
            let nearby_drivers = self.drivers_on_grid.get(position_hash.into());
            let mut driver_locations = get_locations(&nearby_drivers);
            result.append(&mut driver_locations);
        }
        result
    }
}

fn get_locations(stored_drivers: &StorageVec<DriverLocationStorage>) -> Vec<Driver> {
    let mut result = Vec::new();
    for i in 0..stored_drivers.len() {
        result.push(stored_drivers.get(i).unwrap().deref().into())
    }
    result
}

fn closest_driver<'a>(drivers: &'a [Driver], passanger: &Location) -> &'a Driver {
    let mut min = I64F64::max_value();
    let mut result = None;
    for driver in drivers {
        let distance = driver.location.distance_indication(&passanger);
        if distance < min {
            min = distance;
            result = Some(driver)
        }
    }
    return result.unwrap();
}

struct Driver {
    address: Address,
    location: Location,
}
struct Location {
    pub lat: I64F64,
    pub lon: I64F64,
}

impl From<&DriverLocationStorage> for Driver {
    fn from(value: &DriverLocationStorage) -> Self {
        Driver {
            address: value.address.get(),
            location: value.location(),
        }
    }
}

impl Location {
    pub fn from_i128_tuple(t: (i128, i128)) -> Self {
        Location {
            lat: to_fixed(t.0),
            lon: to_fixed(t.1),
        }
    }

    pub fn distance_indication(&self, other: &Location) -> I64F64 {
        let lat_diff = self.lat - other.lat;
        let lon_diff = self.lon - other.lon;
        lat_diff.abs() + lon_diff.abs()
    }
}

#[inline]
fn to_fixed(x: i128) -> I64F64 {
    I64F64::from_be_bytes(x.to_be_bytes())
}

#[inline]
fn to_fixed_signed(x: Signed<128, 2>) -> I64F64 {
    I64F64::from_be_bytes(x.to_be_bytes())
}

fn encode_geohash(location: &Location) -> GeoHash<5> {
    GeoHash::<5>::try_from_params(location.lat, location.lon).unwrap()
}

fn all_neighbors(center: &GeoHash<5>) -> Vec<GeoHash<5>> {
    let mut result = Vec::new();
    let neighbors = center.neighbors().expect("Invalid hash");
    result.push(neighbors.n);
    result.push(neighbors.ne);
    result.push(neighbors.e);
    result.push(neighbors.se);
    result.push(neighbors.s);
    result.push(neighbors.sw);
    result.push(neighbors.w);
    result.push(neighbors.nw);
    result
}

pub enum GeocabError {
    InvalidGeohashLength(InvalidGeohashLength),
    GenericError(GenericError),
}

sol! {
    event TripBooked(address indexed passenger, address indexed driver, int128 dest_lat, int128 dest_lon);

    error InvalidTokenId(uint256 token_id);
    error NotOwner(address from, uint256 token_id, address real_owner);
    error NotApproved(uint256 token_id, address owner, address spender);
    error TransferToZero(uint256 token_id);
    error ReceiverRefused(address receiver, uint256 token_id, bytes4 returned);
    error InvalidGeohashLength();
    error GenericError();
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use alloc::{string::String, vec::Vec};
    use substrate_fixed::types::I64F64;
    use substrate_geohash::GeoHash;

    use super::{encode_geohash, Location};

    #[test]
    fn test_geohash() {
        let hash: String =
            encode_geohash(&Location::from_i128_tuple((940783947759187132416, 0))).into();
        assert_eq!("gcpfp", hash)
    }

    #[test]
    fn test_geo_2() {
        let lat = I64F64::from_str("51.0").unwrap();
        let lon = I64F64::from_str("0.0").unwrap();
        let geohash = GeoHash::<5>::try_from_params(lat, lon).unwrap();
        let neighbors = geohash.neighbors().unwrap();
        let geo_str = geohash.into();
        let mut results = Vec::<String>::new();
        results.push(geo_str);
        results.push(neighbors.n.into());
        results.push(neighbors.ne.into());
        results.push(neighbors.e.into());
        results.push(neighbors.se.into());
        results.push(neighbors.s.into());
        results.push(neighbors.sw.into());
        results.push(neighbors.w.into());
        results.push(neighbors.nw.into());
        assert_eq!(
            "gcpfp, gcpfr, u1042, u1040, u101b, gcpcz, gcpcy, gcpfn, gcpfq",
            results.join(", ")
        );
    }

    #[test]
    fn test_num() {
        let n = 51_i128 << 64;
        assert_eq!(940783947759187132416, n)
    }
}
