use mecomp_storage::db::schemas::{album, artist, collection, playlist, song, Id, Thing};

pub fn parse_things_from_lines<Lines>(lines: Lines) -> Vec<Thing>
where
    Lines: Iterator<Item = String>,
{
    lines.fold(Vec::new(), |mut acc, line| {
        // deserialize the thing from the string
        // the line should follow the pattern:
        // <table_name>:<26 character, upperalphanumeric id>
        // anything else should be considered invalid, and ignored
        //
        // input may also look like:
        //     <table_name>:<26 character, upperalphanumeric id>: <some other text>
        // this is okay too, the extra text will be ignored
        let parts: Vec<&str> = line.trim().split(':').collect();

        if parts.len() >= 2 {
            let tb = parts[0];
            let id = parts[1];

            if (matches!(
                tb,
                artist::TABLE_NAME
                    | album::TABLE_NAME
                    | song::TABLE_NAME
                    | playlist::TABLE_NAME
                    | collection::TABLE_NAME
            )) && id.len() == 26
                && id
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
            {
                acc.push(Thing {
                    tb: tb.to_owned(),
                    id: Id::String(id.to_owned()),
                });
            }
        }

        acc
    })
}
