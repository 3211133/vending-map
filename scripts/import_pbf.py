#!/usr/bin/env python3
"""Import OSM vending machine data from a PBF file into PostgreSQL."""

import argparse
import sys
from typing import Callable, List, Optional, Sequence, Tuple


try:
    import osmium
except ImportError:
    print(
        "Missing required package: osmium. Install it with `pip install osmium`.",
        file=sys.stderr,
    )
    sys.exit(1)

try:
    import psycopg2
    from psycopg2.extras import Json, execute_values
except ImportError:
    print(
        "Missing required package: psycopg2-binary. Install it with "
        "`pip install psycopg2-binary`.",
        file=sys.stderr,
    )
    sys.exit(1)


BATCH_SIZE = 1000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Import vending machine nodes from an OSM PBF into PostgreSQL."
    )
    parser.add_argument("--pbf", required=True, help="Path to the OSM PBF file")
    parser.add_argument("--db", required=True, help="PostgreSQL DATABASE_URL")
    return parser.parse_args()


def extract_vending_type(tags: dict) -> str:
    return tags.get("vending") or tags.get("vending:type") or "drinks"


Row = Tuple[int, float, float, Optional[str], str, Optional[str], Optional[str], Json]


def row_from_node(node: "osmium.osm.Node") -> Row:
    tags = dict(node.tags)
    return (
        node.id,
        node.location.lon,
        node.location.lat,
        tags.get("name"),
        extract_vending_type(tags),
        tags.get("operator"),
        tags.get("opening_hours"),
        Json(tags),
    )


class VendingMachineHandler(osmium.SimpleHandler):
    def __init__(self, flush_batch: Callable[[Sequence[Row]], None]) -> None:
        super().__init__()
        self.flush_batch = flush_batch
        self.batch: List[Row] = []

    def node(self, node: "osmium.osm.Node") -> None:
        if not node.location.valid():
            return

        tags = node.tags
        if tags.get("amenity") != "vending_machine":
            return

        self.batch.append(row_from_node(node))
        if len(self.batch) >= BATCH_SIZE:
            self.flush()

    def flush(self) -> None:
        if not self.batch:
            return
        self.flush_batch(self.batch)
        self.batch = []


def upsert_batch(cursor, batch: Sequence[Row]) -> Tuple[int, int]:
    query = """
        INSERT INTO vending_machines (
            osm_id,
            geom,
            name,
            vending_type,
            operator,
            opening_hours,
            source,
            osm_tags,
            is_active,
            updated_at
        )
        VALUES %s
        ON CONFLICT (osm_id) DO UPDATE
        SET
            geom = EXCLUDED.geom,
            name = COALESCE(EXCLUDED.name, vending_machines.name),
            vending_type = EXCLUDED.vending_type,
            operator = COALESCE(EXCLUDED.operator, vending_machines.operator),
            opening_hours = COALESCE(EXCLUDED.opening_hours, vending_machines.opening_hours),
            source = 'osm',
            osm_tags = EXCLUDED.osm_tags,
            is_active = TRUE,
            updated_at = NOW()
        RETURNING (xmax = 0) AS inserted
    """

    template = """(
        %s,
        ST_SetSRID(ST_MakePoint(%s, %s), 4326),
        %s,
        %s,
        %s,
        %s,
        'osm',
        %s,
        TRUE,
        NOW()
    )"""

    execute_values(cursor, query, batch, template=template, page_size=len(batch))
    results = cursor.fetchall()
    inserted = sum(1 for (is_inserted,) in results if is_inserted)
    updated = len(results) - inserted
    return inserted, updated


def main() -> int:
    args = parse_args()

    inserted_total = 0
    updated_total = 0
    processed_total = 0

    conn = psycopg2.connect(args.db)
    try:
        conn.autocommit = False
        with conn, conn.cursor() as cursor:
            def flush_batch(batch: Sequence[Row]) -> None:
                nonlocal inserted_total, updated_total, processed_total
                inserted, updated = upsert_batch(cursor, batch)
                inserted_total += inserted
                updated_total += updated
                processed_total += len(batch)

                if processed_total % BATCH_SIZE == 0:
                    print(
                        f"Processed {processed_total} records "
                        f"(inserted: {inserted_total}, updated: {updated_total})"
                    )

            handler = VendingMachineHandler(flush_batch)
            print(f"Reading PBF: {args.pbf}")
            handler.apply_file(args.pbf, locations=True)
            handler.flush()

        print(
            f"Import complete. Processed {processed_total} records "
            f"(inserted: {inserted_total}, updated: {updated_total})"
        )
    except Exception as exc:
        conn.rollback()
        print(f"Import failed: {exc}", file=sys.stderr)
        return 1
    finally:
        conn.close()

    return 0


if __name__ == "__main__":
    sys.exit(main())
