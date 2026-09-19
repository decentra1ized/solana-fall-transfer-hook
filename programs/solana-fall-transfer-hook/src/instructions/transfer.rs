// The transfer_checked CPI lives in the separate `transfer-helper` program.
// Keeping it separate avoids the forbidden call chain:
// transfer-hook -> Token-2022 -> transfer-hook.
