// pest. The Elegant Parser
// Copyright (C) 2017  Dragoș Tiselice
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A `mod` containing constructs useful in infix operator parsing with the precedence climbing
//! method.

use std::collections::HashMap;
use std::iter::Peekable;
use std::ops::BitOr;

use super::inputs::Input;
use super::iterators::Pair;
use super::RuleType;

/// An `enum` describing an `Operator`'s associativity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Assoc {
    /// Left `Operator` associativity
    Left,
    /// Right `Operator` associativity
    Right
}

/// A `struct` defining an infix operator used in [`PrecClimber`](struct.PrecClimber.html).
#[derive(Debug)]
pub struct Operator<R: RuleType> {
    rule: R,
    assoc: Assoc,
    next: Option<Box<Operator<R>>>
}

impl<R: RuleType> Operator<R> {
    /// Creates a new `Operator` from a `Rule` and `Assoc`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pest::prec_climber::{Assoc, Operator};
    /// # #[allow(non_camel_case_types)]
    /// # #[allow(dead_code)]
    /// # #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    /// # enum Rule {
    /// #     plus,
    /// #     minus
    /// # }
    /// let additive = Operator::new(Rule::plus, Assoc::Left) |
    ///                Operator::new(Rule::minus, Assoc::Right);
    /// ```
    pub fn new(rule: R, assoc: Assoc) -> Operator<R> {
        Operator {
            rule,
            assoc,
            next: None
        }
    }
}

impl<R: RuleType> BitOr for Operator<R> {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self {
        self.next = Some(Box::new(rhs));
        self
    }
}

/// A `struct` useful in order to perform [precedence climbing][1] on infix expressions contained in
/// a [`Pairs`](../iterators/struct.Pairs.html). The token pairs contained in the `Pairs` should
/// start with a *primary* pair and then alternate between an *operator* and a *primary*.
///
/// [1]: https://en.wikipedia.org/wiki/Operator-precedence_parser#Precedence_climbing_method
#[derive(Debug)]
pub struct PrecClimber<R: RuleType> {
    ops: HashMap<R, (u32, Assoc)>
}

impl<R: RuleType> PrecClimber<R> {
    /// Creates a new `PrecClimber` from the `Operator`s contained in `ops`. Every entry in the
    /// `Vec` has precedence *index + 1*. In order to have operators with same precedence, they need
    /// to be chained with `|` between them.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pest::prec_climber::{Assoc, Operator, PrecClimber};
    /// # #[allow(non_camel_case_types)]
    /// # #[allow(dead_code)]
    /// # #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    /// # enum Rule {
    /// #     plus,
    /// #     minus,
    /// #     times,
    /// #     divide,
    /// #     power
    /// # }
    /// let climber = PrecClimber::new(vec![
    ///     Operator::new(Rule::plus, Assoc::Left) | Operator::new(Rule::minus, Assoc::Left),
    ///     Operator::new(Rule::times, Assoc::Left) | Operator::new(Rule::divide, Assoc::Left),
    ///     Operator::new(Rule::power, Assoc::Right)
    /// ]);
    /// ```
    pub fn new(ops: Vec<Operator<R>>) -> PrecClimber<R> {
        let ops = ops.into_iter().zip(1..).fold(HashMap::new(), |mut map, (op, prec)| {
            let mut next = Some(op);

            while let Some(op) = next.take() {
                match op {
                    Operator { rule, assoc, next: op_next } => {
                        map.insert(rule, (prec, assoc));
                        next = op_next.map(|op| *op);
                    }
                }
            }

            map
        });

        PrecClimber {
            ops
        }
    }

    /// Performs the precedence climbing algorithm on the `pairs` in a similar manner to map-reduce.
    /// *Primary* pairs are mapped with `primary` and then reduced to one single result with
    /// `infix`.
    ///
    /// # Panics
    ///
    /// Panics will occur when `pairs` is empty or when the alternating *primary*, *operator*,
    /// *primary* order is not respected.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let primary = |pair| {
    ///     consume(pair, climber)
    /// };
    /// let infix = |lhs: i32, op: Pair<Rule, StringInput>, rhs: i32| {
    ///     match op.rule() {
    ///         Rule::plus => lhs + rhs,
    ///         Rule::minus => lhs - rhs,
    ///         Rule::times => lhs * rhs,
    ///         Rule::divide => lhs / rhs,
    ///         Rule::power => lhs.pow(rhs as u32),
    ///         _ => unreachable!()
    ///     }
    /// };
    ///
    /// let result = climber.climb(pairs, primary, infix);
    /// ```
    pub fn climb<I: Input, P, F, G, T>(
        &self,
        mut pairs: P,
        mut primary: F,
        mut infix: G
    ) -> T
    where
        P: Iterator<Item=Pair<R, I>>,
        F: FnMut(Pair<R, I>) -> T,
        G: FnMut(T, Pair<R, I>, T) -> T
    {
        let lhs = primary(pairs.next().expect("precedence climbing requires a non-empty Pairs"));
        self.climb_rec(lhs, 0, &mut pairs.peekable(), &mut primary, &mut infix)
    }

    fn climb_rec<I: Input, P, F, G, T>(
        &self,
        mut lhs: T,
        min_prec: u32,
        pairs: &mut Peekable<P>,
        primary: &mut F,
        infix: &mut G
    ) -> T
    where
        P: Iterator<Item=Pair<R, I>>,
        F: FnMut(Pair<R, I>) -> T,
        G: FnMut(T, Pair<R, I>, T) -> T
    {
        while pairs.peek().is_some() {
            let rule = pairs.peek().unwrap().as_rule();
            if let Some(&(prec, _)) = self.ops.get(&rule) {
                if prec >= min_prec {
                    let op = pairs.next().unwrap();
                    let mut rhs = primary(pairs.next().expect("infix operator must be followed by \
                                                               a primary expression"));

                    while pairs.peek().is_some() {
                        let rule = pairs.peek().unwrap().as_rule();
                        if let Some(&(new_prec, assoc)) = self.ops.get(&rule) {
                            if new_prec > prec || assoc == Assoc::Right && new_prec == prec {
                                rhs = self.climb_rec(rhs, new_prec, pairs, primary, infix);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    lhs = infix(lhs, op, rhs);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        lhs
    }
}
