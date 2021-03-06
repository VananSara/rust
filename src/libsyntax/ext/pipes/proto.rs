// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast;
use codemap::span;
use ext::base::ExtCtxt;
use ext::build::AstBuilder;
use ext::pipes::ast_builder::{append_types, path};

#[deriving(Eq)]
pub enum direction { send, recv }

impl ToStr for direction {
    fn to_str(&self) -> ~str {
        match *self {
          send => ~"Send",
          recv => ~"Recv"
        }
    }
}

impl direction {
    pub fn reverse(&self) -> direction {
        match *self {
          send => recv,
          recv => send
        }
    }
}

pub struct next_state {
    state: @str,
    tys: ~[@ast::Ty],
}

// name, span, data, current state, next state
pub struct message(@str, span, ~[@ast::Ty], state, Option<next_state>);

impl message {
    pub fn name(&mut self) -> @str {
        match *self {
          message(id, _, _, _, _) => id
        }
    }

    pub fn span(&mut self) -> span {
        match *self {
          message(_, span, _, _, _) => span
        }
    }

    /// Return the type parameters actually used by this message
    pub fn get_generics(&self) -> ast::Generics {
        match *self {
          message(_, _, _, this, _) => copy this.generics
        }
    }
}

pub type state = @state_;

pub struct state_ {
    id: uint,
    name: @str,
    ident: ast::ident,
    span: span,
    dir: direction,
    generics: ast::Generics,
    messages: @mut ~[message],
    proto: protocol
}

impl state_ {
    pub fn add_message(@self,
                       name: @str,
                       span: span,
                       data: ~[@ast::Ty],
                       next: Option<next_state>) {
        self.messages.push(message(name, span, data, self,
                                   next));
    }

    pub fn filename(&self) -> ~str {
        self.proto.filename()
    }

    pub fn data_name(&self) -> ast::ident {
        self.ident
    }

    /// Returns the type that is used for the messages.
    pub fn to_ty(&self, cx: @ExtCtxt) -> @ast::Ty {
        cx.ty_path
            (path(~[cx.ident_of(self.name)],self.span).add_tys(
                cx.ty_vars(&self.generics.ty_params)), @None)
    }

    /// Iterate over the states that can be reached in one message
    /// from this state.
    pub fn reachable(&self, f: &fn(state) -> bool) -> bool {
        for self.messages.iter().advance |m| {
            match *m {
              message(_, _, _, _, Some(next_state { state: ref id, _ })) => {
                let state = self.proto.get_state((*id));
                if !f(state) { return false; }
              }
              _ => ()
            }
        }
        return true;
    }
}

pub type protocol = @mut protocol_;

pub fn protocol(name: @str, span: span) -> protocol {
    @mut protocol_(name, span)
}

pub fn protocol_(name: @str, span: span) -> protocol_ {
    protocol_ {
        name: name,
        span: span,
        states: @mut ~[],
        bounded: None
    }
}

pub struct protocol_ {
    name: @str,
    span: span,
    states: @mut ~[state],

    bounded: Option<bool>,
}

impl protocol_ {
    /// Get a state.
    pub fn get_state(&self, name: &str) -> state {
        *self.states.iter().find_(|i| name == i.name).get()
    }

    pub fn get_state_by_id(&self, id: uint) -> state { self.states[id] }

    pub fn has_state(&self, name: &str) -> bool {
        self.states.iter().find_(|i| name == i.name).is_some()
    }

    pub fn filename(&self) -> ~str {
        ~"proto://" + self.name
    }

    pub fn num_states(&self) -> uint {
        let states = &mut *self.states;
        states.len()
    }

    pub fn has_ty_params(&self) -> bool {
        for self.states.iter().advance |s| {
            if s.generics.ty_params.len() > 0 {
                return true;
            }
        }
        false
    }

    pub fn is_bounded(&self) -> bool {
        let bounded = self.bounded.get();
        bounded
    }
}

impl protocol_ {
    pub fn add_state_poly(@mut self,
                          name: @str,
                          ident: ast::ident,
                          dir: direction,
                          generics: ast::Generics)
                          -> state {
        let messages = @mut ~[];
        let states = &mut *self.states;

        let state = @state_ {
            id: states.len(),
            name: name,
            ident: ident,
            span: self.span,
            dir: dir,
            generics: generics,
            messages: messages,
            proto: self
        };

        states.push(state);
        state
    }
}

pub trait visitor<Tproto, Tstate, Tmessage> {
    fn visit_proto(&self, proto: protocol, st: &[Tstate]) -> Tproto;
    fn visit_state(&self, state: state, m: &[Tmessage]) -> Tstate;
    fn visit_message(&self, name: @str, spane: span, tys: &[@ast::Ty],
                     this: state, next: Option<next_state>) -> Tmessage;
}

pub fn visit<Tproto, Tstate, Tmessage, V: visitor<Tproto, Tstate, Tmessage>>(
    proto: protocol, visitor: V) -> Tproto {

    // the copy keywords prevent recursive use of dvec
    let states: ~[Tstate] = do (copy proto.states).iter().transform |&s| {
        let messages: ~[Tmessage] = do (copy s.messages).iter().transform |&m| {
            let message(name, span, tys, this, next) = m;
            visitor.visit_message(name, span, tys, this, next)
        }.collect();
        visitor.visit_state(s, messages)
    }.collect();
    visitor.visit_proto(proto, states)
}
