// Copyright 2022 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use accesskit::{ActionHandler, ActivationHandler, TreeUpdate};
use objc2::{
    ClassType, DeclaredClass, declare_class,
    ffi::{
        IMP, OBJC_ASSOCIATION_RETAIN_NONATOMIC, class_getInstanceMethod, class_getSuperclass,
        class_replaceMethod, method_getImplementation, method_getTypeEncoding, objc_class,
        objc_getAssociatedObject, objc_setAssociatedObject, object_getClass,
    },
    msg_send_id,
    mutability::InteriorMutable,
    rc::Id,
    runtime::{AnyClass, Sel},
    sel,
};
use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::{NSArray, NSObject, NSPoint};
use std::{
    cell::RefCell,
    ffi::{CStr, c_void},
    sync::Mutex,
};

use crate::{Adapter, event::QueuedEvents};

// zgx patch: the view's class is never changed (ZGX_PATCH.md, "No class
// change"). The accessibility methods go on the view's own class instead,
// once, and a view without an adapter answers as its class did before.

/// A class given the accessibility methods, and what it answered before.
struct Patched {
    class: usize,
    children: IMP,
    focus: IMP,
    hit_test: IMP,
}

static PATCHED: Mutex<Vec<Patched>> = Mutex::new(Vec::new());

static ASSOCIATED_OBJECT_KEY: u8 = 0;

fn associated_object_key() -> *const c_void {
    (&ASSOCIATED_OBJECT_KEY as *const u8).cast()
}

struct AssociatedObjectState {
    adapter: Adapter,
    activation_handler: Box<dyn ActivationHandler>,
}

struct AssociatedObjectIvars {
    state: RefCell<AssociatedObjectState>,
}

declare_class!(
    struct AssociatedObject;

    unsafe impl ClassType for AssociatedObject {
        type Super = NSObject;
        type Mutability = InteriorMutable;
        const NAME: &'static str = "AccessKitSubclassAssociatedObject";
    }

    impl DeclaredClass for AssociatedObject {
        type Ivars = AssociatedObjectIvars;
    }
);

impl AssociatedObject {
    fn new(adapter: Adapter, activation_handler: impl 'static + ActivationHandler) -> Id<Self> {
        let state = RefCell::new(AssociatedObjectState {
            adapter,
            activation_handler: Box::new(activation_handler),
        });
        let this = Self::alloc().set_ivars(AssociatedObjectIvars { state });

        unsafe { msg_send_id![super(this), init] }
    }
}

/// The view's adapter: none for a view of a patched class that has no
/// adapter (it never had one, or its adapter was dropped).
fn associated_object(view: &NSView) -> Option<&AssociatedObject> {
    unsafe {
        (objc_getAssociatedObject(view as *const NSView as *const _, associated_object_key())
            as *const AssociatedObject)
            .as_ref()
    }
}

/// What the view's class answered before it was patched.
fn original(view: &NSView, pick: fn(&Patched) -> IMP) -> IMP {
    let patched = PATCHED.lock().unwrap();
    let mut class = unsafe { object_getClass(view as *const NSView as *const _) };
    while !class.is_null() {
        if let Some(entry) = patched.iter().find(|entry| entry.class == class as usize) {
            return pick(entry);
        }
        class = unsafe { class_getSuperclass(class) };
    }
    None
}

unsafe extern "C" fn children(this: &NSView, cmd: Sel) -> *mut NSArray<NSObject> {
    let Some(associated) = associated_object(this) else {
        let Some(imp) = original(this, |entry| entry.children) else {
            return std::ptr::null_mut();
        };
        let imp: unsafe extern "C" fn(&NSView, Sel) -> *mut NSArray<NSObject> =
            unsafe { std::mem::transmute(imp) };
        return unsafe { imp(this, cmd) };
    };
    let mut state = associated.ivars().state.borrow_mut();
    let state_mut = &mut *state;
    state_mut
        .adapter
        .view_children(&mut *state_mut.activation_handler)
}

unsafe extern "C" fn focus(this: &NSView, cmd: Sel) -> *mut NSObject {
    let Some(associated) = associated_object(this) else {
        let Some(imp) = original(this, |entry| entry.focus) else {
            return std::ptr::null_mut();
        };
        let imp: unsafe extern "C" fn(&NSView, Sel) -> *mut NSObject =
            unsafe { std::mem::transmute(imp) };
        return unsafe { imp(this, cmd) };
    };
    let mut state = associated.ivars().state.borrow_mut();
    let state_mut = &mut *state;
    state_mut.adapter.focus(&mut *state_mut.activation_handler)
}

unsafe extern "C" fn hit_test(this: &NSView, cmd: Sel, point: NSPoint) -> *mut NSObject {
    let Some(associated) = associated_object(this) else {
        let Some(imp) = original(this, |entry| entry.hit_test) else {
            return std::ptr::null_mut();
        };
        let imp: unsafe extern "C" fn(&NSView, Sel, NSPoint) -> *mut NSObject =
            unsafe { std::mem::transmute(imp) };
        return unsafe { imp(this, cmd, point) };
    };
    let mut state = associated.ivars().state.borrow_mut();
    let state_mut = &mut *state;
    state_mut
        .adapter
        .hit_test(point, &mut *state_mut.activation_handler)
}

/// Puts `imp` on `class` for `sel`, and returns what the class answered
/// before (its own method or the one it inherited).
unsafe fn replace(class: *const objc_class, sel: Sel, imp: IMP, types: &CStr) -> IMP {
    unsafe {
        let method = class_getInstanceMethod(class, sel.as_ptr());
        let (original, types) = if method.is_null() {
            (None, types.as_ptr())
        } else {
            (
                method_getImplementation(method),
                method_getTypeEncoding(method),
            )
        };
        class_replaceMethod(class as *mut objc_class, sel.as_ptr(), imp, types);
        original
    }
}

/// Gives the view's class the accessibility methods, once per class.
fn patch_class(view: &NSView) {
    let mut class = unsafe { object_getClass(view as *const NSView as *const _) };
    // A class key-value observing made for this object goes when its
    // observers do: the class under it is the view's own.
    while !class.is_null()
        && unsafe { &*(class as *const AnyClass) }
            .name()
            .starts_with("NSKVONotifying_")
    {
        class = unsafe { class_getSuperclass(class) };
    }
    if class.is_null() {
        return;
    }
    let mut patched = PATCHED.lock().unwrap();
    // A subclass of a patched class inherits the methods.
    let mut ancestor = class;
    while !ancestor.is_null() {
        if patched.iter().any(|entry| entry.class == ancestor as usize) {
            return;
        }
        ancestor = unsafe { class_getSuperclass(ancestor) };
    }
    let entry = unsafe {
        Patched {
            class: class as usize,
            children: replace(
                class,
                sel!(accessibilityChildren),
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSView, Sel) -> *mut NSArray<NSObject>,
                    IMP,
                >(children),
                c"@@:",
            ),
            focus: replace(
                class,
                sel!(accessibilityFocusedUIElement),
                std::mem::transmute::<unsafe extern "C" fn(&NSView, Sel) -> *mut NSObject, IMP>(
                    focus,
                ),
                c"@@:",
            ),
            hit_test: replace(
                class,
                sel!(accessibilityHitTest:),
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSView, Sel, NSPoint) -> *mut NSObject,
                    IMP,
                >(hit_test),
                c"@@:{CGPoint=dd}",
            ),
        }
    };
    patched.push(entry);
}

/// Implements the `NSView` accessibility methods on the view's class when
/// normal subclassing isn't an option (zgx patch: without changing the
/// class of the view).
pub struct SubclassingAdapter {
    view: Id<NSView>,
    associated: Id<AssociatedObject>,
}

impl SubclassingAdapter {
    /// Create an adapter for the specified view.
    ///
    /// The action handler will always be called on the main thread.
    ///
    /// # Safety
    ///
    /// `view` must be a valid, unreleased pointer to an `NSView`.
    pub unsafe fn new(
        view: *mut c_void,
        activation_handler: impl 'static + ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Self {
        let view = view as *mut NSView;
        let retained_view = unsafe { Id::retain(view) }.unwrap();
        Self::new_internal(retained_view, activation_handler, action_handler)
    }

    fn new_internal(
        retained_view: Id<NSView>,
        activation_handler: impl 'static + ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Self {
        let view = Id::as_ptr(&retained_view) as *mut NSView;
        if !unsafe {
            objc_getAssociatedObject(view as *const NSView as *const _, associated_object_key())
        }
        .is_null()
        {
            panic!("subclassing adapter already instantiated on view {view:?}");
        }
        let adapter = unsafe { Adapter::new(view as *mut c_void, false, action_handler) };
        let associated = AssociatedObject::new(adapter, activation_handler);
        unsafe {
            objc_setAssociatedObject(
                view as *mut _,
                associated_object_key(),
                Id::as_ptr(&associated) as *mut _,
                OBJC_ASSOCIATION_RETAIN_NONATOMIC,
            )
        };
        // zgx patch: the methods go on the view's class; the view keeps it.
        patch_class(&retained_view);
        Self {
            view: retained_view,
            associated,
        }
    }

    /// Create an adapter for the content view of the specified window.
    ///
    /// The action handler will always be called on the main thread.
    ///
    /// # Safety
    ///
    /// `window` must be a valid, unreleased pointer to an `NSWindow`.
    ///
    /// # Panics
    ///
    /// This function panics if the specified window doesn't currently have
    /// a content view.
    pub unsafe fn for_window(
        window: *mut c_void,
        activation_handler: impl 'static + ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Self {
        let window = unsafe { &*(window as *const NSWindow) };
        let retained_view = window.contentView().unwrap();
        Self::new_internal(retained_view, activation_handler, action_handler)
    }

    /// If and only if the tree has been initialized, call the provided function
    /// and apply the resulting update. Note: If the caller's implementation of
    /// [`ActivationHandler::request_initial_tree`] initially returned `None`,
    /// the [`TreeUpdate`] returned by the provided function must contain
    /// a full tree.
    ///
    /// If a [`QueuedEvents`] instance is returned, the caller must call
    /// [`QueuedEvents::raise`] on it.
    pub fn update_if_active(
        &mut self,
        update_factory: impl FnOnce() -> TreeUpdate,
    ) -> Option<QueuedEvents> {
        let mut state = self.associated.ivars().state.borrow_mut();
        state.adapter.update_if_active(update_factory)
    }

    /// Update the tree state based on whether the window is focused.
    ///
    /// If a [`QueuedEvents`] instance is returned, the caller must call
    /// [`QueuedEvents::raise`] on it.
    pub fn update_view_focus_state(&mut self, is_focused: bool) -> Option<QueuedEvents> {
        let mut state = self.associated.ivars().state.borrow_mut();
        state.adapter.update_view_focus_state(is_focused)
    }
}

impl Drop for SubclassingAdapter {
    fn drop(&mut self) {
        // zgx patch: the view keeps its class, which AppKit may be observing
        // it under; without its adapter it answers as its class did before.
        let view = Id::as_ptr(&self.view) as *mut NSView;
        unsafe {
            objc_setAssociatedObject(
                view as *mut _,
                associated_object_key(),
                std::ptr::null_mut(),
                OBJC_ASSOCIATION_RETAIN_NONATOMIC,
            )
        };
    }
}
