use mecomp_storage::db::schemas::Thing;

pub fn parse_things_from_lines<Lines>(lines: Lines) -> Vec<Thing>
where
    Lines: Iterator<Item = String>,
{
    lines.fold(Vec::new(), |mut acc, line| {
        if let Ok(thing) = line.parse() {
            acc.push(thing);
        }
        acc
    })
}
