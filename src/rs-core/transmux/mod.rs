// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

mod elementary_packet_parser;
mod exp_golomb;
mod nal_unit_producer;
mod transport_packet_parser;
mod transport_stream_splitter;
