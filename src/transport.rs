use std::time::Duration;
use std::usize;

use libp2p::{self, PeerId, Transport, mplex, secio, yamux};
use libp2p::core::{either, upgrade, transport::boxed::Boxed, StreamMuxer, muxing::StreamMuxerBox};
use libp2p::transport_timeout::TransportTimeout;

/// Builds the transport that serves as a common ground for all connections.
pub fn build_transport(
	local_private_key: secio::SecioKeyPair
) -> Boxed<(PeerId, StreamMuxerBox)> {
	let mut mplex_config = mplex::MplexConfig::new();
	mplex_config.max_buffer_len_behaviour(mplex::MaxBufferBehaviour::Block);
	mplex_config.max_buffer_len(usize::MAX);

	let base = libp2p::CommonTransport::new()
		.with_upgrade(secio::SecioConfig {
			key: local_private_key,
		})
		.and_then(move |out, endpoint, client_addr| {
			let upgrade = upgrade::or(
				upgrade::map(yamux::Config::default(), either::EitherOutput::First),
				upgrade::map(mplex_config, either::EitherOutput::Second),
			);
			let peer_id = out.remote_key.into_peer_id();
			let upgrade = upgrade::map(upgrade, move |muxer| (peer_id, muxer));
			upgrade::apply(out.stream, upgrade, endpoint, client_addr)
		})
		.map(|(id, muxer), _| (id, muxer.boxed()));

	TransportTimeout::new(base, Duration::from_secs(20))
		.boxed()
}
