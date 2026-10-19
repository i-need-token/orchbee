//! Globally unique exact hex suffixes; unshortenable identities stay copyable full IDs.
pub(super) fn short(kind: &str, ids: &[&str]) -> Vec<String> {
    let mut order: Vec<_> = (0..ids.len()).collect();
    order.sort_by(|a, b| {
        ids[*a]
            .as_bytes()
            .iter()
            .rev()
            .cmp(ids[*b].as_bytes().iter().rev())
    });
    let mut shared = vec![0; ids.len()];
    for position in 0..order.len() {
        let index = order[position];
        for neighbor in [
            position.checked_sub(1),
            position.checked_add(1).filter(|p| *p < order.len()),
        ]
        .into_iter()
        .flatten()
        {
            let common = ids[index]
                .as_bytes()
                .iter()
                .rev()
                .zip(ids[order[neighbor]].as_bytes().iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            shared[index] = shared[index].max(common);
        }
    }
    ids.iter()
        .enumerate()
        .map(|(i, id)| {
            let n = shared[i] + 1;
            if n <= 31 && n <= id.len() {
                format!("@{kind}:{}", &id[id.len() - n..])
            } else {
                (*id).to_owned()
            }
        })
        .collect()
}
