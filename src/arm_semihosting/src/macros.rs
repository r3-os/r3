/// Variable argument version of `syscall`
#[macro_export]
macro_rules! syscall {
    ($nr:ident) => {
        $crate::syscall1($crate::nr::$nr, 0)
    };
    ($nr:ident, $a1:expr) => {
        $crate::syscall($crate::nr::$nr, &[$a1 as usize])
    };
    ($nr:ident, $a1:expr, $a2:expr) => {
        $crate::syscall($crate::nr::$nr, &[$a1 as usize, $a2 as usize])
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr) => {
        $crate::syscall($crate::nr::$nr, &[$a1 as usize, $a2 as usize, $a3 as usize])
    };
    ($nr:ident, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::syscall(
            $crate::nr::$nr,
            &[$a1 as usize, $a2 as usize, $a3 as usize, $a4 as usize],
        )
    };
}

/// Macro version of `syscall1`
#[macro_export]
macro_rules! syscall1 {
    ($nr:ident, $a1:expr) => {
        $crate::syscall1($crate::nr::$nr, $a1 as usize)
    };
}

/// Macro for printing to the HOST standard output
///
/// This macro returns a `Result<(), ()>` value
#[macro_export]
macro_rules! hprint {
    ($($tt:tt)*) => {
        match ::core::format_args!($($tt)*) {
            args => if let ::core::option::Option::Some(s) = args.as_str() {
                $crate::export::hstdout_str(s)
            } else {
                $crate::export::hstdout_fmt(args)
            },
        }
    };
}

/// Macro for printing to the HOST standard output, with a newline.
///
/// This macro returns a `Result<(), ()>` value
#[macro_export]
macro_rules! hprintln {
    ($($tt:tt)*) => {
        match $crate::hprint!($($tt)*) {
            Ok(()) => $crate::export::hstdout_str("\n"),
            Err(()) => Err(()),
        }
    };
}

/// Macro for printing to the HOST standard error
///
/// This macro returns a `Result<(), ()>` value
#[macro_export]
macro_rules! heprint {
    ($($tt:tt)*) => {
        match ::core::format_args!($($tt)*) {
            args => if let ::core::option::Option::Some(s) = args.as_str() {
                $crate::export::hstderr_str(s)
            } else {
                $crate::export::hstderr_fmt(args)
            },
        }
    };
}

/// Macro for printing to the HOST standard error, with a newline.
///
/// This macro returns a `Result<(), ()>` value
#[macro_export]
macro_rules! heprintln {
    ($($tt:tt)*) => {
        match $crate::heprint!($($tt)*) {
            Ok(()) => $crate::export::hstderr_str("\n"),
            Err(()) => Err(()),
        }
    };
}

/// Macro that prints and returns the value of a given expression
/// for quick and dirty debugging. Works exactly like `dbg!` in
/// the standard library, replacing `eprintln` with `heprintln`,
/// which it unwraps.
#[macro_export]
macro_rules! dbg {
    () => {
        $crate::heprintln!("[{}:{}]", file!(), line!()).unwrap();
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::heprintln!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp).unwrap();
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { $crate::dbg!($val) };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}
