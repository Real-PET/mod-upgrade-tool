// https://loup-vaillant.fr/tutorials/earley-parsing/recogniser

use super::{Ambiguity, CompletedEarleyItem, EarleyItem, EarleyRecognizerResult};
use crate::{ASTNodeLabel, Rule, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct EarleyRecognizer<'parser, Label: ASTNodeLabel> {
    rules: &'parser [Rule<Label>],
    nullables: &'parser HashMap<Label, &'parser Rule<Label>>,
    sets: Vec<Vec<EarleyItem<'parser, Label>>>,
    ambiguities: Vec<Rc<RefCell<Ambiguity<'parser, Label>>>>,
}

impl<'parser, Label: ASTNodeLabel> EarleyRecognizer<'parser, Label> {
    pub fn new(
        nullables: &'parser HashMap<Label, &'parser Rule<Label>>,
        rules: &'parser [Rule<Label>],
    ) -> Self {
        Self {
            rules,
            nullables,
            sets: Vec::new(),
            ambiguities: Vec::new(),
        }
    }

    fn create_ambiguity(&mut self) -> Rc<RefCell<Ambiguity<'parser, Label>>> {
        let ambiguity = Rc::new(RefCell::new(Ambiguity::new()));
        self.ambiguities.push(ambiguity.clone());
        ambiguity
    }

    pub fn recognize<'a>(
        mut self,
        entry: Label,
        tokens: &'parser [Token<'a, Label>],
    ) -> EarleyRecognizerResult<'parser, Label> {
        let mut first_set = Vec::new();

        // initialization
        for rule in self.rules.iter() {
            if rule.label == entry {
                first_set.push(EarleyItem::new(0, rule, self.create_ambiguity()));
            }
        }

        self.sets.push(first_set);

        // primary loop
        // using indexes as the sets will grow during the loop
        let mut i = 0;

        // scanning can create new sets
        while i < self.sets.len() {
            let mut j = 0;

            // prediction + completion adds to the current set
            while j < self.sets[i].len() {
                let label = self.sets[i][j].next_label();

                match label {
                    None => self.complete(i, j),
                    Some(label) => {
                        self.predict(i, j, label);
                        self.scan(i, j, label, tokens);
                    }
                }

                j += 1
            }

            i += 1
        }

        EarleyRecognizerResult::new(self.sets, self.ambiguities, self.nullables)
    }

    fn complete(&mut self, i: usize, j: usize) {
        let item = &self.sets[i][j];
        let completed_item = item.as_completed_item(i);

        let new_items: Vec<_> = self.sets[completed_item.start]
            .iter_mut()
            .filter(|old_item| old_item.next_label() == Some(completed_item.rule.label))
            .map(|old_item| {
                old_item.add_completed_item(completed_item.clone());
                old_item.advance()
            })
            .collect();

        for item in new_items {
            self.append_if_unique(i, item);
        }
    }

    fn scan<'a>(&mut self, i: usize, j: usize, label: Label, tokens: &[Token<'a, Label>]) {
        if tokens.get(i).map(|token| token.label) == Some(label) {
            let item = &self.sets[i][j];

            let new_item = item.advance();

            if self.sets.len() <= i + 1 {
                self.sets.push(vec![new_item]);
            } else {
                self.sets[i + 1].push(new_item);
            }
        }
    }

    fn predict(&mut self, i: usize, j: usize, label: Label) {
        for rule in self.rules.iter() {
            if rule.label != label {
                continue;
            }

            let ambiguity = self.create_ambiguity();
            self.append_if_unique(i, EarleyItem::new(i, rule, ambiguity));
        }

        // https://loup-vaillant.fr/tutorials/earley-parsing/empty-rules
        // "magic completion", Aycock & Horspool's nullable rule solution
        if let Some(rule) = self.nullables.get(&label) {
            let item = &mut self.sets[i][j];

            // create a new item
            let completed_item = CompletedEarleyItem::new_nullable(rule, i);
            item.add_completed_item(completed_item);

            let advanced_item = item.advance();
            self.append_if_unique(i, advanced_item);
        }
    }

    fn append_if_unique(&mut self, i: usize, item: EarleyItem<'parser, Label>) {
        let set = &mut self.sets[i];

        if !set.contains(&item) {
            set.push(item);
        }
    }
}
