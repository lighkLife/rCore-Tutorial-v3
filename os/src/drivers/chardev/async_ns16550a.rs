use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::task::Poll::{Pending, Ready};

use bitflags::*;
use volatile::{ReadOnly, Volatile, WriteOnly};

use crate::sync::UPIntrFreeCell;

///! Ref: https://www.lammertbies.nl/comm/info/serial-uart
///! Ref: ns16550a datasheet: https://datasheetspdf.com/pdf-file/605590/NationalSemiconductor/NS16550A/1
///! Ref: ns16450 datasheet: https://datasheetspdf.com/pdf-file/1311818/NationalSemiconductor/NS16450/1
use super::CharDevice;

bitflags! {
    /// InterruptEnableRegister
    pub struct IER: u8 {
        const RX_AVAILABLE = 1 << 0;
        const TX_EMPTY = 1 << 1;
    }

    /// LineStatusRegister
    pub struct LSR: u8 {
        const DATA_AVAILABLE = 1 << 0;
        const THR_EMPTY = 1 << 5;
    }

    /// Model Control Register
    pub struct MCR: u8 {
        const DATA_TERMINAL_READY = 1 << 0;
        const REQUEST_TO_SEND = 1 << 1;
        const AUX_OUTPUT1 = 1 << 2;
        const AUX_OUTPUT2 = 1 << 3;
    }
}

#[repr(C)]
#[allow(dead_code)]
struct ReadWithoutDLAB {
    /// receiver buffer register
    pub rbr: ReadOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<IER>,
    /// interrupt identification register
    pub iir: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// model control register
    pub mcr: Volatile<MCR>,
    /// line status register
    pub lsr: ReadOnly<LSR>,
    /// ignore MSR
    _padding1: ReadOnly<u8>,
    /// ignore SCR
    _padding2: ReadOnly<u8>,
}

#[repr(C)]
#[allow(dead_code)]
struct WriteWithoutDLAB {
    /// transmitter holding register
    pub thr: WriteOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<IER>,
    /// ignore FCR
    _padding0: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// modem control register
    pub mcr: Volatile<MCR>,
    /// line status register
    pub lsr: ReadOnly<LSR>,
    /// ignore other registers
    _padding1: ReadOnly<u16>,
}

pub struct NS16550aRaw {
    base_addr: usize,
    waker_list: VecDeque<Waker>,
}

impl NS16550aRaw {
    fn read_end(&mut self) -> &mut ReadWithoutDLAB {
        unsafe { &mut *(self.base_addr as *mut ReadWithoutDLAB) }
    }

    fn write_end(&mut self) -> &mut WriteWithoutDLAB {
        unsafe { &mut *(self.base_addr as *mut WriteWithoutDLAB) }
    }

    pub fn new(base_addr: usize) -> Self {
        Self { base_addr, waker_list: VecDeque::new() }
    }

    pub fn init(&mut self) {
        let read_end = self.read_end();
        let mut mcr = MCR::empty();
        mcr |= MCR::DATA_TERMINAL_READY;
        mcr |= MCR::REQUEST_TO_SEND;
        mcr |= MCR::AUX_OUTPUT2;
        read_end.mcr.write(mcr);
        let ier = IER::RX_AVAILABLE;
        read_end.ier.write(ier);
    }

    pub fn read(self: &Arc<Self>) -> AsyncCharReader {
        AsyncCharReader {
            raw: self.clone()
        }
    }

    pub fn write(self: &Arc<Self>, ch: u8) -> AsyncCharWriter {
        AsyncCharWriter {
            raw: self.clone(),
            ch,
        }
    }
}


struct AsyncCharWriter {
    raw: Arc<NS16550aRaw>,
    ch: u8,
}

impl Future for AsyncCharWriter {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let write_end = self.raw.write_end();
        if write_end.lsr.read().contains(LSR::THR_EMPTY) {
            // writable
            write_end.thr.write(self.ch);
            Ready(())
        } else {
            let waker = cx.waker().clone();
            self.raw.waker_list.push_back(waker);
            Pending
        }
    }
}

struct AsyncCharReader {
    raw: Arc<NS16550aRaw>,
}

impl Future for AsyncCharReader {
    type Output = u8;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let read_end = self.raw.read_end();
        if read_end.lsr.read().contains(LSR::DATA_AVAILABLE) {
            // writable
            let ch = read_end.rbr.read();
            Ready(ch)
        } else {
            let waker = cx.waker().clone();
            self.raw.waker_list.push_back(waker);
            Pending
        }
    }
}


pub struct AsyncNS16550a<const BASE_ADDR: usize> {
    inner: UPIntrFreeCell<NS16550aRaw>,
}

impl<const BASE_ADDR: usize> AsyncNS16550a<BASE_ADDR> {
    pub fn new() -> Self {
        let inner = NS16550aRaw::new(BASE_ADDR);
        //inner.ns16550a.init();
        Self {
            inner: unsafe { UPIntrFreeCell::new(inner) },
        }
    }

    pub fn read_buffer_is_empty(&self) -> bool {
        true
    }
}

impl<const BASE_ADDR: usize> CharDevice for AsyncNS16550a<BASE_ADDR> {
    fn init(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.init();
        drop(inner);
    }

    async fn read(&self) -> u8 {
        let mut inner = self.inner.exclusive_access();
        inner.read().await
    }
    async fn write(&self, ch: u8) {
        let mut inner = self.inner.exclusive_access();
        inner.write(ch).await
    }

    fn handle_irq(&self) {
        self.inner.exclusive_session(|inner| {
            if let Some(waker) = inner.waker_list.pop_front() {
                waker.clone().wake();
            }
        });
    }
}

