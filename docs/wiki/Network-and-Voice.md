# Network, chat, and voice status

## What works now

- VATSIM CID/password verification requests a short-lived FSD token. A
  successful verification does not log the pilot into an FSD server.
- The FSD decoder handles bounded text, ping/pong, and controller updates.
- The text core can classify incoming private messages by sender callsign,
  frequency messages, and server messages. It can prepare `.msg CALLSIGN text`
  and frequency text packets and rejects packet delimiter injection.
- Audio settings can select devices and test the local microphone. These are
  local tests, not network voice calls.

## Still required for an end-to-end client

1. Obtain an authorized ovPilot VATSIM client identity and verify the current
   FSD login procedure with VATSIM. Reusing another client's identity would
   misidentify this application.
2. Add a real FSD connection lifecycle, server selection, reconnect,
   authentication challenges, position updates, and disconnect handling.
3. Route incoming `#TM` packets to the main log and private conversation UI.
   An incoming `.msg` must open or update the sender's conversation; outgoing
   `.msg` must be shown as sent only after the network write succeeds.
4. Read COM1/COM2 frequencies and transmit selection from a live simulator.
   Frequency text and private messages must use those actual values and the
   connected callsign.
5. Implement and verify the current VATSIM voice protocol. TX must indicate
   actual outgoing voice on the selected frequency, and RX actual incoming
   voice, with sender callsigns where the protocol supplies them. A pressed
   PTT key alone is not evidence of network transmission.
6. Integrate simulator aircraft state, transponder, remote aircraft, model
   matching, and flight plans for each supported simulator and OS.

There is no valid live VATSIM or voice end-to-end test in this repository yet.
The main Connect, TX, and RX controls remain disabled until their transport
and simulator inputs can be verified.
