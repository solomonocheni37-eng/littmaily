use crate::util::ical::parse_ical_field;

/// Extracts fields from raw vCard text.
///
/// vCard (RFC 6350) and iCalendar (RFC 5545) share the exact same line-folding
/// and property-parameter parsing rules, so we safely reuse the iCal parser.
pub fn parse_vcard_field(vcard: &str, field: &str) -> Option<String> {
    parse_ical_field(vcard, field)
}
