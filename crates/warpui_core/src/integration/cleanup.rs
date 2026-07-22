use std::cell::RefCell;
use std::rc::Rc;

use super::TestSetupUtils;
use crate::platform::app::AppCallbacks;

pub type SetupFn = Box<dyn FnMut(&mut TestSetupUtils) + 'static>;

struct TestCleanup {
    test_setup: TestSetupUtils,
    callback: Option<SetupFn>,
}

#[derive(Clone)]
pub(super) struct TestCleanupHandle(Rc<RefCell<TestCleanup>>);

impl TestCleanupHandle {
    pub(super) fn new(test_setup: TestSetupUtils, callback: SetupFn) -> Self {
        Self(Rc::new(RefCell::new(TestCleanup {
            test_setup,
            callback: Some(callback),
        })))
    }

    pub(super) fn run(&self) {
        let mut cleanup = self.0.borrow_mut();
        cleanup.test_setup.cleanup_env();
        if let Some(mut callback) = cleanup.callback.take() {
            callback(&mut cleanup.test_setup);
        }
    }

    pub(super) fn install_on_termination(&self, callbacks: &mut AppCallbacks) {
        let cleanup = self.clone();
        let mut on_will_terminate = callbacks.on_will_terminate.take();
        callbacks.on_will_terminate = Some(Box::new(move |ctx| {
            if let Some(callback) = &mut on_will_terminate {
                callback(ctx);
            }
            cleanup.run();
        }));
    }
}
