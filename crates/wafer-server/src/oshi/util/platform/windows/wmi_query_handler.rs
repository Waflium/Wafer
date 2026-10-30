use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoInitializeSecurity, CoUninitialize, EOAC_NONE,
    RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_I4, VariantClear};
use windows::Win32::System::Wmi::{IWbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE, WbemLocator};
use windows::core::{BSTR, HRESULT, PCWSTR};

const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
const RPC_E_TOO_LATE: i32 = 0x8001_0119u32 as i32;
const WBEM_E_INVALID_NAMESPACE: i32 = 0x8004_100Eu32 as i32;
const WBEM_E_INVALID_CLASS: i32 = 0x8004_1010u32 as i32;
const WBEM_E_INVALID_QUERY: i32 = 0x8004_1017u32 as i32;
const NAMESPACE: &str = "ROOT\\CIMV2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WmiValue {
    Null,
    String(String),
    Int(i32),
}

#[derive(Default)]
pub struct WmiResult {
    pub rows: Vec<Vec<WmiValue>>,
}

impl WmiResult {
    pub fn get_result_count(&self) -> usize {
        self.rows.len()
    }

    pub fn get_string(&self, row: usize, column: usize) -> String {
        match &self.rows[row][column] {
            WmiValue::String(value) => value.clone(),
            _ => String::new(),
        }
    }

    pub fn get_uint64(&self, row: usize, column: usize) -> i64 {
        match &self.rows[row][column] {
            WmiValue::String(value) => value.parse().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn get_int(&self, row: usize, column: usize) -> i32 {
        match &self.rows[row][column] {
            WmiValue::Int(value) => *value,
            _ => 0,
        }
    }
}

fn query_to_string(class_name: &str, properties: &[&str]) -> String {
    format!("SELECT {} FROM {class_name}", properties.join(","))
}

fn init_com() -> windows::core::Result<bool> {
    let mut threading = COINIT_MULTITHREADED;
    for _ in 0..2 {
        let result = unsafe { CoInitializeEx(None, threading) };
        if result.0 == RPC_E_CHANGED_MODE {
            threading = if threading == COINIT_MULTITHREADED { COINIT_APARTMENTTHREADED } else { COINIT_MULTITHREADED };
            continue;
        }
        result.ok()?;
        let security = unsafe { CoInitializeSecurity(None, -1, None, None, RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE, None) };
        if let Err(error) = security
            && error.code().0 != RPC_E_TOO_LATE
        {
            unsafe { CoUninitialize() };
            return Err(error);
        }
        return Ok(true);
    }
    Ok(false)
}

fn read_value(variant: &VARIANT) -> WmiValue {
    let inner = unsafe { &variant.Anonymous.Anonymous };
    match inner.vt {
        VT_BSTR => WmiValue::String(unsafe { &inner.Anonymous.bstrVal }.to_string()),
        VT_I4 => WmiValue::Int(unsafe { inner.Anonymous.lVal }),
        _ => WmiValue::Null,
    }
}

fn execute(class_name: &str, properties: &[&str]) -> windows::core::Result<Vec<Vec<WmiValue>>> {
    let locator: IWbemLocator = unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)? };
    let services = unsafe { locator.ConnectServer(&BSTR::from(NAMESPACE), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)? };
    let enumerator = unsafe {
        services.ExecQuery(&BSTR::from("WQL"), &BSTR::from(query_to_string(class_name, properties)), WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, None)?
    };
    let mut rows = Vec::new();
    loop {
        let mut objects = [None];
        let mut returned = 0u32;
        let result = unsafe { enumerator.Next(WBEM_INFINITE, &mut objects, &mut returned) };
        result.ok()?;
        let Some(object) = objects[0].take().filter(|_| returned > 0) else { break };
        let mut row = Vec::with_capacity(properties.len());
        for property in properties {
            let name: Vec<u16> = property.encode_utf16().chain(std::iter::once(0)).collect();
            let mut variant = VARIANT::default();
            unsafe { object.Get(PCWSTR(name.as_ptr()), 0, &mut variant, None, None)? };
            row.push(read_value(&variant));
            unsafe { VariantClear(&mut variant)? };
        }
        rows.push(row);
    }
    Ok(rows)
}

fn handle_com_exception(class_name: &str, properties: &[&str], hresult: HRESULT, message: &str) {
    match hresult.0 {
        WBEM_E_INVALID_NAMESPACE => warn!("COM exception: Invalid Namespace {NAMESPACE}"),
        WBEM_E_INVALID_CLASS => warn!("COM exception: Invalid Class {class_name}"),
        WBEM_E_INVALID_QUERY => warn!("COM exception: Invalid Query: {}", query_to_string(class_name, properties)),
        code => warn!("COM exception querying {class_name}, which might not be on your system. Will not attempt to query it again. Error was {code}: {message}"),
    }
}

pub fn query_wmi(class_name: &str, properties: &[&str]) -> WmiResult {
    let mut com_init = false;
    let outcome = init_com().and_then(|initialized| {
        com_init = initialized;
        execute(class_name, properties)
    });
    if com_init {
        unsafe { CoUninitialize() };
    }
    match outcome {
        Ok(rows) => WmiResult { rows },
        Err(error) => {
            handle_com_exception(class_name, properties, error.code(), &error.message());
            WmiResult::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_rendered_like_wmi_query_to_string() {
        assert_eq!(query_to_string("Win32_PhysicalMemory", &["CAPACITY", "SPEED"]), "SELECT CAPACITY,SPEED FROM Win32_PhysicalMemory");
    }

    #[test]
    fn operating_system_properties_can_be_queried() {
        let result = query_wmi("Win32_OperatingSystem", &["BUILDNUMBER", "SUITEMASK"]);
        assert_eq!(result.get_result_count(), 1);
        assert!(result.get_string(0, 0).parse::<u32>().is_ok());
    }
}
