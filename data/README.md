# Aircraft type data

`aircraft_types.json` is a deduplicated, alphabetically sorted copy of the
`type_designator`, `manufacturer`, and `model` columns from
[`ColtJD45/icao-aircraft-designator-list`](https://github.com/ColtJD45/icao-aircraft-designator-list),
retrieved on 2026-10-29. The upstream dataset is distributed under the MIT
license in `aircraft_types-LICENSE`.

The bundled catalog supplies offline suggestions. It does not establish that
every existing or future ICAO designator is present, and users may still enter
codes that are absent from the catalog.
