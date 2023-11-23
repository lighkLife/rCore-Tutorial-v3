pub mod thread {
    use core::marker::PhantomData;
    use core::sync::atomic::{AtomicBool, Ordering};

    // use portable_atomic::{AtomicBool, Ordering};

    use embassy_executor::{raw, Spawner};
    use riscv::_export::critical_section;

    /// global atomic used to keep track of whether there is work to do since sev() is not available on RISCV
    static SIGNAL_WORK_THREAD_MODE: AtomicBool = AtomicBool::new(false);

    static SIGNAL_WORK_FINISH: AtomicBool = AtomicBool::new(false);


    #[export_name = "__pender"]
    fn __pender(_context: *mut ()) {
        SIGNAL_WORK_THREAD_MODE.store(true, Ordering::SeqCst);
    }

    #[export_name = "__work_finish"]
    fn __work_finish() {
        SIGNAL_WORK_FINISH.store(true, Ordering::SeqCst);
    }

    #[derive(Clone, Copy)]
    pub struct WorkMarker();

    unsafe impl Send for WorkMarker {}
    unsafe impl Sync for WorkMarker {}

    impl WorkMarker {
        pub(crate) fn mark_finish(self) {
            extern "Rust" {
                fn __work_finish();
            }
            unsafe { __work_finish() };
        }
    }

    /// RISCV32 Executor
    pub struct Executor {
        inner: raw::Executor,
        not_send: PhantomData<*mut ()>,
    }

    impl Executor {
        /// Create a new Executor.
        pub fn new() -> Self {
            Self {
                inner: raw::Executor::new(core::ptr::null_mut()),
                not_send: PhantomData,
            }
        }

        /// Run the executor.
        ///
        /// The `init` closure is called with a [`Spawner`] that spawns tasks on
        /// this executor. Use it to spawn the initial task(s). After `init` returns,
        /// the executor starts running the tasks.
        ///
        /// To spawn more tasks later, you may keep copies of the [`Spawner`] (it is `Copy`),
        /// for example by passing it as an argument to the initial tasks.
        ///
        /// This function requires `&'static mut self`. This means you have to store the
        /// Executor instance in a place where it'll live forever and grants you mutable
        /// access. There's a few ways to do this:
        ///
        /// - a [StaticCell](https://docs.rs/static_cell/latest/static_cell/) (safe)
        /// - a `static mut` (unsafe)
        /// - a local variable in a function you know never returns (like `fn main() -> !`), upgrading its lifetime with `transmute`. (unsafe)
        ///
        /// This function never returns.
        pub fn run(&'static mut self, init: impl FnOnce(Spawner)) {
            init(self.inner.spawner());

            loop {
                unsafe {
                    println!("executor poll 1");
                    self.inner.poll();
                    println!("executor poll 2");
                    if SIGNAL_WORK_FINISH.load(Ordering::SeqCst) {
                        //work finish
                        println!("work finish");
                        break;
                    }
                    println!("executor poll 3");
                    // we do not care about race conditions between the load and store operations, interrupts
                    //will only set this value to true.
                    // critical_section::with(|_| {
                        println!("executor poll 4");
                        // if there is work to do, loop back to polling
                        // TODO can we relax this?
                        if SIGNAL_WORK_THREAD_MODE.load(Ordering::SeqCst) {
                            println!("executor poll 5");
                            SIGNAL_WORK_THREAD_MODE.store(false, Ordering::SeqCst);
                        }
                        // if not, wait for interrupt
                        else {
                            println!("executor poll 6");
                            core::arch::asm!("wfi");
                        }
                    // });
                    // if an interrupt occurred while waiting, it will be serviced here
                }
            }
        }
    }
}
