/**
 * Travel Module
 * Trip/leg/booking/place entities; timeline + map views (issue #62,
 * docs/MODULES.md §3.7). Trip/leg/place are plain user-authored entities.
 * Actually purchasing a flight/hotel is HUMAN-GATED (docs/SECURITY.md §2) -
 * it only ever creates a `pending_approval` draft, never books directly.
 */
osRegisterModule({
  id: "travel",
  name: "Travel",
  icon: "Plane",
  color: "var(--neo-blue)",
  num: 12,
  version: "1.0.0",

  entityTypes: {
    trip: {
      label: "Trip",
      plural: "Trips",
      icon: "Map",
      attrs: {
        name: { type: "text", required: true },
        start: { type: "date", required: false },
        end: { type: "date", required: false },
        destination: { type: "text", required: false },
        budget: { type: "number", required: false },
        status: { type: "text", required: false }
      },
      display: {
        title: "title",
        subtitle: "start"
      }
    },
    leg: {
      label: "Leg",
      plural: "Legs",
      icon: "Navigation",
      attrs: {
        kind: { type: "enum", enum: ["flight", "train", "drive"], required: false },
        start: { type: "date", required: false },
        end: { type: "date", required: false }
      },
      display: {
        title: "title",
        subtitle: "kind"
      }
    },
    booking: {
      label: "Booking",
      plural: "Bookings",
      icon: "Ticket",
      attrs: {
        provider: { type: "text", required: false },
        confirmation: { type: "text", required: false },
        cost: { type: "number", required: false },
        file_ref: { type: "blob", required: false }
      },
      display: {
        title: "provider",
        subtitle: "confirmation"
      }
    },
    place: {
      label: "Place",
      plural: "Places",
      icon: "MapPin",
      attrs: {
        category: { type: "text", required: false },
        lat: { type: "number", required: false },
        lng: { type: "number", required: false }
      },
      display: {
        title: "title",
        subtitle: "category"
      }
    }
  },

  views: [
    { id: "trips", label: "Trips", kind: "list", type: "trip" },
    { id: "timeline", label: "Timeline", kind: "timeline", type: "leg" },
    { id: "map", label: "Map", kind: "map", type: "place" },
    { id: "bookings", label: "Bookings", kind: "table", type: "booking" }
  ],

  events: ["trip.created", "booking.added", "itinerary.changed"],

  agentTools: [
    { name: "travel.plan", schema: {}, impl: "plan", gated: false },
    { name: "travel.parse_emails", schema: {}, impl: "parse_emails", gated: false },
    { name: "travel.book", schema: {}, impl: "book", gated: true } // GATED
  ],

  integrations: []
});
