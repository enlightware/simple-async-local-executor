extern crate simple_async_local_executor;
use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use futures::Future;
use simple_async_local_executor::*;

/// A typical game unit with a position.
///
/// In a real game this struct would contain all state of the unit.
#[derive(Default)]
struct Unit {
    /// The 1-D position of the unit. In a real game, it would be a 2D or 3D.
    pub pos: i32,
}
type UnitRef = Rc<RefCell<Unit>>;

/// A future that will move the unit towards `target_pos` at each step,
/// and complete when the unit has reached that position.
struct UnitGotoFuture {
    unit: UnitRef,
    target_pos: i32,
}
impl Future for UnitGotoFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let unit_pos = self.unit.borrow().pos;
        if unit_pos == self.target_pos {
            Poll::Ready(())
        } else {
            self.unit.borrow_mut().pos += (self.target_pos - unit_pos).signum();
            Poll::Pending
        }
    }
}

/// Helper async function to write unit behavior nicely
async fn goto(unit: UnitRef, pos: i32) {
    UnitGotoFuture {
        unit,
        target_pos: pos,
    }
    .await;
}

/// Let a unit go back and forth between two positions
async fn patrol(unit: UnitRef, poses: [i32; 2]) {
    loop {
        goto(unit.clone(), poses[0]).await;
        goto(unit.clone(), poses[1]).await;
    }
}

/// Test program with two units: one patrolling and one going to a position.
fn main() {
    let executor = Executor::default();
    let units: [UnitRef; 2] = Default::default();
    executor.spawn(patrol(units[0].clone(), [-5, 5]));
    executor.spawn(goto(units[1].clone(), 12));
    let print_poses = || {
        println!(
            "Unit poses: {}",
            units
                .iter()
                .map(|unit| unit.borrow().pos.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    print_poses();
    for _ in 0..30 {
        executor.step();
        print_poses();
    }
}
