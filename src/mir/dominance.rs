use crate::mir::BasicBlockId;
use std::collections::{HashMap, HashSet};

pub struct DominanceInfo {
    pub idom: HashMap<BasicBlockId, BasicBlockId>,
    pub dominance_frontiers: HashMap<BasicBlockId, HashSet<BasicBlockId>>,
}

pub struct LoopInfo {
    pub headers: HashSet<BasicBlockId>,
    pub natural_loops: HashMap<BasicBlockId, HashSet<BasicBlockId>>,
    pub pre_headers: HashMap<BasicBlockId, BasicBlockId>,
}

pub fn compute_dominance(
    entry: BasicBlockId,
    predecessors: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    _successors: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    all_blocks: &[BasicBlockId],
) -> DominanceInfo {
    let mut doms: HashMap<BasicBlockId, HashSet<BasicBlockId>> = HashMap::new();
    let all_set: HashSet<BasicBlockId> = all_blocks.iter().cloned().collect();

    for b in all_blocks {
        let b = b.clone();
        if b == entry {
            let mut s = HashSet::new();
            s.insert(entry.clone());
            doms.insert(b, s);
        } else {
            doms.insert(b, all_set.clone());
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for b in all_blocks {
            let b = b.clone();
            if b == entry {
                continue;
            }

            let mut new_dom: HashSet<BasicBlockId> = all_set.clone();
            if let Some(preds) = predecessors.get(&b) {
                for p in preds {
                    if let Some(p_dom) = doms.get(p) {
                        new_dom = new_dom.intersection(p_dom).cloned().collect();
                    }
                }
            }
            new_dom.insert(b.clone());

            if let Some(old_dom) = doms.get(&b) {
                if *old_dom != new_dom {
                    doms.insert(b.clone(), new_dom);
                    changed = true;
                }
            }
        }
    }

    let mut idom = HashMap::new();
    for b in all_blocks {
        let b = b.clone();
        if b == entry {
            continue;
        }
        if let Some(dom_b) = doms.get(&b) {
            let mut strict_doms = dom_b.clone();
            strict_doms.remove(&b);

            let mut idom_b = None;
            for d in &strict_doms {
                let mut is_idom = true;
                for other_d in &strict_doms {
                    if d == other_d {
                        continue;
                    }
                    if let Some(dom_other_d) = doms.get(other_d) {
                        if dom_other_d.contains(d) {
                            is_idom = false;
                            break;
                        }
                    }
                }
                if is_idom {
                    idom_b = Some(d.clone());
                    break;
                }
            }
            if let Some(id) = idom_b {
                idom.insert(b.clone(), id);
            }
        }
    }

    let mut df: HashMap<BasicBlockId, HashSet<BasicBlockId>> = HashMap::new();
    for b in all_blocks {
        df.insert(b.clone(), HashSet::new());
    }

    for b in all_blocks {
        if let Some(preds) = predecessors.get(b) {
            if preds.len() >= 2 {
                for p in preds {
                    let mut runner = p.clone();
                    while Some(&runner) != idom.get(b) && runner != *b {
                        df.entry(runner.clone()).or_default().insert(b.clone());
                        if let Some(next_runner) = idom.get(&runner) {
                            runner = next_runner.clone();
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    DominanceInfo {
        idom,
        dominance_frontiers: df,
    }
}

pub fn detect_loops(
    _entry: BasicBlockId,
    predecessors: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    successors: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    all_blocks: &[BasicBlockId],
    dominance: &DominanceInfo,
) -> LoopInfo {
    let mut back_edges = Vec::new();
    for n in all_blocks {
        if let Some(succs) = successors.get(n) {
            for d in succs {
                let mut dominates = false;
                let mut curr = n.clone();
                if curr == *d {
                    dominates = true;
                } else {
                    while let Some(id) = dominance.idom.get(&curr) {
                        if *id == *d {
                            dominates = true;
                            break;
                        }
                        if *id == curr {
                            break;
                        }
                        curr = id.clone();
                    }
                }

                if dominates {
                    back_edges.push((n.clone(), d.clone()));
                }
            }
        }
    }

    let mut headers = HashSet::new();
    let mut natural_loops = HashMap::new();

    for (n, d) in &back_edges {
        headers.insert(d.clone());

        let mut loop_nodes = HashSet::new();
        loop_nodes.insert(d.clone());
        loop_nodes.insert(n.clone());

        let mut stack = vec![n.clone()];
        while let Some(m) = stack.pop() {
            if let Some(preds) = predecessors.get(&m) {
                for p in preds {
                    if !loop_nodes.contains(p) {
                        loop_nodes.insert(p.clone());
                        stack.push(p.clone());
                    }
                }
            }
        }

        let current_loop_nodes = natural_loops.entry(d.clone()).or_insert_with(HashSet::new);
        current_loop_nodes.extend(loop_nodes);
    }

    let mut pre_headers = HashMap::new();
    for h in &headers {
        if let Some(preds) = predecessors.get(h) {
            let outside_preds: Vec<BasicBlockId> = preds
                .iter()
                .filter(|&p| {
                    if let Some(l) = natural_loops.get(h) {
                        !l.contains(p)
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            if outside_preds.len() == 1 {
                pre_headers.insert(h.clone(), outside_preds[0].clone());
            }
        }
    }

    LoopInfo {
        headers,
        natural_loops,
        pre_headers,
    }
}
