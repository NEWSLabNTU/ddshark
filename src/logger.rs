use rustdds::{
    structure::guid::{EntityId, GuidPrefix},
    GUID,
};
use serde::Serialize;

use crate::{
    state::{ReaderState, State, TopicState, WriterState},
    utils::{GUIDExt, GuidPrefixExt},
};
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{self, BufWriter},
    path::{Path, PathBuf},
};

type CsvWriter = csv::Writer<File>;

#[derive(Debug)]
pub struct Logger {
    log_dir: PathBuf,
    topic_dir: PathBuf,
    participant_dir: PathBuf,
    participants: HashMap<GuidPrefix, ParticipantLogger>,
    topics: HashMap<String, TopicLogger>,
}

impl Logger {
    pub fn new() -> io::Result<Self> {
        let cwd = env::current_dir().unwrap();
        let log_dir = cwd.join("ddshark");

        if log_dir.exists() {
            let old_dir = (1..)
                .find_map(|idx| {
                    let old_dir = cwd.join(format!("ddshark.old.{idx}"));
                    let ok = !old_dir.exists();
                    ok.then_some(old_dir)
                })
                .unwrap();
            fs::rename(&log_dir, old_dir).unwrap();
        }

        let topic_dir = log_dir.join("topic");
        let participant_dir = log_dir.join("participant");

        fs::create_dir(&log_dir).unwrap();
        fs::create_dir(&participant_dir).unwrap();
        fs::create_dir(&topic_dir).unwrap();
        Ok(Self {
            log_dir,
            topic_dir,
            participant_dir,
            participants: HashMap::new(),
            topics: HashMap::new(),
        })
    }

    pub fn save(&mut self, state: &State) -> io::Result<()> {
        use std::collections::hash_map::Entry as E;

        for (&guid_prefix, part_state) in &state.participants {
            let part_logger = match self.participants.entry(guid_prefix) {
                E::Occupied(entry) => entry.into_mut(),
                E::Vacant(entry) => {
                    let participant_dir = self
                        .participant_dir
                        .join(format!("{}", guid_prefix.display()));
                    let writer_dir = participant_dir.join("writers");
                    let reader_dir = participant_dir.join("readers");
                    fs::create_dir(&participant_dir).unwrap();
                    fs::create_dir(&writer_dir).unwrap();
                    fs::create_dir(&reader_dir).unwrap();

                    let logger = ParticipantLogger {
                        writer_dir,
                        reader_dir,
                        writers: HashMap::new(),
                        readers: HashMap::new(),
                        participant_dir,
                    };
                    entry.insert(logger)
                }
            };

            for (&writer_id, writer_state) in &part_state.writers {
                let guid = GUID::new(guid_prefix, writer_id);

                let writer_logger = match part_logger.writers.entry(writer_id) {
                    E::Occupied(entry) => entry.into_mut(),
                    E::Vacant(entry) => {
                        let log_path = part_logger
                            .writer_dir
                            .join(format!("{}.csv", guid.display()));
                        let writer = create_writer(log_path).unwrap();
                        let logger = WriterLogger { writer };
                        entry.insert(logger)
                    }
                };

                let WriterState {
                    last_sn,
                    total_msg_count,
                    total_byte_count,
                    avg_msgrate,
                    avg_bitrate,
                    ref data,
                    ..
                } = *writer_state;

                let topic_name = data
                    .as_ref()
                    .map(|data| data.publication_topic_data.topic_name.clone());

                let record = WriterRecord {
                    last_sn: last_sn.map(|sn| sn.0),
                    total_msg_count,
                    total_byte_count,
                    avg_msgrate,
                    avg_bitrate,
                    topic_name,
                };
                writer_logger.writer.serialize(record).unwrap();
            }

            for (&reader_id, reader_state) in &part_state.readers {
                let guid = GUID::new(guid_prefix, reader_id);

                let reader_logger = match part_logger.readers.entry(reader_id) {
                    E::Occupied(entry) => entry.into_mut(),
                    E::Vacant(entry) => {
                        let log_path = part_logger
                            .reader_dir
                            .join(format!("{}.csv", guid.display()));
                        let writer = create_writer(log_path).unwrap();
                        let logger = ReaderLogger { writer };
                        entry.insert(logger)
                    }
                };

                let ReaderState {
                    last_sn,
                    total_acknack_count,
                    avg_acknack_rate,
                    ..
                } = *reader_state;

                let record = ReaderRecord {
                    last_sn,
                    total_acknack_count,
                    avg_acknack_rate,
                };
                reader_logger.writer.serialize(record).unwrap();
            }

            for (topic_name, topic_state) in &state.topics {
                let TopicState { readers, writers } = topic_state;
                let n_readers = readers.len();
                let n_writers = writers.len();

                let topic_logger = match self.topics.entry(topic_name.clone()) {
                    E::Occupied(entry) => entry.into_mut(),
                    E::Vacant(entry) => {
                        let name = topic_name.replace('/', "|");
                        let file_name = format!("{name}.csv");
                        let path = self.topic_dir.join(file_name);
                        let writer = create_writer(path).unwrap();
                        let logger = TopicLogger { writer };

                        entry.insert(logger)
                    }
                };

                let record = TopicRecord {
                    n_readers,
                    n_writers,
                };

                topic_logger.writer.serialize(record).unwrap();
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct WriterLogger {
    pub writer: CsvWriter,
}

#[derive(Debug)]
struct ReaderLogger {
    pub writer: CsvWriter,
}

#[derive(Debug)]
struct ParticipantLogger {
    pub participant_dir: PathBuf,
    pub writer_dir: PathBuf,
    pub reader_dir: PathBuf,
    pub writers: HashMap<EntityId, WriterLogger>,
    pub readers: HashMap<EntityId, ReaderLogger>,
}

#[derive(Debug)]
struct TopicLogger {
    pub writer: CsvWriter,
}

#[derive(Debug, Serialize)]
struct ParticipantRecord {}

#[derive(Debug, Serialize)]
struct WriterRecord {
    pub last_sn: Option<i64>,
    pub total_msg_count: usize,
    pub total_byte_count: usize,
    pub avg_msgrate: f64,
    pub avg_bitrate: f64,
    pub topic_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReaderRecord {
    pub last_sn: Option<i64>,
    pub total_acknack_count: usize,
    pub avg_acknack_rate: f64,
}

#[derive(Debug, Serialize)]
struct TopicRecord {
    pub n_readers: usize,
    pub n_writers: usize,
}

fn create_writer<P>(path: P) -> io::Result<CsvWriter>
where
    P: AsRef<Path>,
{
    let writer = File::create(path).unwrap();
    let csv_wtr = csv::Writer::from_writer(writer);
    Ok(csv_wtr)
}
