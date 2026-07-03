import React from 'react';
import { MapContainer, TileLayer, Marker, Popup } from 'react-leaflet';
import 'leaflet/dist/leaflet.css';
import L from 'leaflet';
import iconUrl from 'leaflet/dist/images/marker-icon.png';
import iconRetinaUrl from 'leaflet/dist/images/marker-icon-2x.png';
import shadowUrl from 'leaflet/dist/images/marker-shadow.png';
import { resolveField, resolveDisplay } from './displayHelpers';

// Leaflet ships marker icon URLs relative to its own package, which Vite's
// bundler breaks unless the assets are re-pointed explicitly.
const markerIcon = L.icon({ iconUrl, iconRetinaUrl, shadowUrl, iconSize: [25, 41], iconAnchor: [12, 41] });

// Generic map renderer: place pins (Travel places, anything with lat/lng
// attrs) over OpenStreetMap tiles - no API key required. Driven by manifest
// display config + a {lat, lng} field pair, never a per-module map component.
export default function GenericMap({ entities, display = {}, latField = 'lat', lngField = 'lng', emptyLabel = 'No located entities yet.' }) {
  const points = (entities || [])
    .map((entity) => ({
      entity,
      lat: Number(resolveField(entity, latField)),
      lng: Number(resolveField(entity, lngField)),
    }))
    .filter((p) => Number.isFinite(p.lat) && Number.isFinite(p.lng));

  if (!points.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  const center = [points[0].lat, points[0].lng];

  return (
    <div className="neo-border overflow-hidden" style={{ height: 420 }}>
      <MapContainer center={center} zoom={4} style={{ height: '100%', width: '100%' }}>
        <TileLayer
          attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
          url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        />
        {points.map(({ entity, lat, lng }) => {
          const { title, subtitle } = resolveDisplay(entity, display);
          return (
            <Marker key={entity.id} position={[lat, lng]} icon={markerIcon}>
              <Popup>
                <strong>{title}</strong>
                {subtitle && <div>{subtitle}</div>}
              </Popup>
            </Marker>
          );
        })}
      </MapContainer>
    </div>
  );
}
