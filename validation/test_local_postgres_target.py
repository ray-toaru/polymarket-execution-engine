#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import check_local_postgres_target


class LocalPostgresTargetTests(unittest.TestCase):
    def test_load_env_file_expands_local_references(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            env_file = Path(tmp_name) / ".env"
            env_file.write_text(
                "\n".join(
                    [
                        "PMX_HOST=127.0.0.1",
                        "PMX_DATABASE_URL=postgres://pmx:pmx@${PMX_HOST}:5432/pmx",
                    ]
                )
                + "\n"
            )
            with patch.dict("check_local_postgres_target.os.environ", {}, clear=True):
                check_local_postgres_target.load_env_file(env_file)
                self.assertEqual(
                    check_local_postgres_target.os.environ["PMX_DATABASE_URL"],
                    "postgres://pmx:pmx@127.0.0.1:5432/pmx",
                )

    def test_database_url_loads_validation_env_before_runtime_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_name:
            tmp = Path(tmp_name)
            (tmp / ".env").write_text("PMX_DATABASE_URL=postgres://pmx@localhost/pmx\n")
            (tmp / ".env.validation").write_text("PMX_TEST_DATABASE_URL=postgres://pmx@localhost/pmx_test\n")
            with patch.object(check_local_postgres_target, "ROOT", tmp):
                with patch.dict("check_local_postgres_target.os.environ", {}, clear=True):
                    self.assertEqual(
                        check_local_postgres_target.database_url(),
                        "postgres://pmx@localhost/pmx_test",
                    )

    def test_parse_database_target(self) -> None:
        self.assertEqual(
            check_local_postgres_target.parse_database_target("postgres://pmx:secret@127.0.0.1:5433/pmx"),
            {
                "scheme": "postgres",
                "hostname": "127.0.0.1",
                "port": 5433,
                "database": "pmx",
                "username": "pmx",
            },
        )

    def test_matching_cluster_by_port(self) -> None:
        cluster = check_local_postgres_target.matching_cluster(
            {"port": 5433},
            [
                {"version": "16", "cluster": "main", "port": "5433", "status": "down"},
                {"version": "15", "cluster": "test", "port": "5432", "status": "online"},
            ],
        )
        self.assertEqual(cluster, {"version": "16", "cluster": "main", "port": "5433", "status": "down"})

    def test_recommendations_for_down_cluster(self) -> None:
        actions = check_local_postgres_target.recommendations(
            {"hostname": "127.0.0.1", "port": 5433, "database": "pmx", "username": "pmx"},
            {"version": "16", "cluster": "main", "port": "5433", "status": "down"},
            {"returncode": 2},
        )
        self.assertTrue(any("pg_ctlcluster 16 main start" in item for item in actions))
        self.assertTrue(any("pg_isready still fails" in item for item in actions))

    @patch("check_local_postgres_target.pg_isready")
    @patch("check_local_postgres_target.pg_lsclusters")
    @patch("check_local_postgres_target.database_url")
    def test_inspect_reports_fail_for_down_cluster(self, database_url_mock, pg_lsclusters_mock, pg_isready_mock) -> None:
        database_url_mock.return_value = "postgres://pmx@127.0.0.1:5433/pmx"
        pg_lsclusters_mock.return_value = [
            {
                "version": "16",
                "cluster": "main",
                "port": "5433",
                "status": "down",
                "owner": "nobody",
                "data_directory": "/var/lib/postgresql/16/main",
                "log_file": "/var/log/postgresql/postgresql-16-main.log",
            }
        ]
        pg_isready_mock.return_value = {"returncode": 2, "stdout": "127.0.0.1:5433 - no response", "stderr": "<empty>"}
        result = check_local_postgres_target.inspect()
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["database_target"]["port"], 5433)
        self.assertEqual(result["matched_cluster"]["cluster"], "main")

    @patch("check_local_postgres_target.pg_isready")
    @patch("check_local_postgres_target.pg_lsclusters")
    @patch("check_local_postgres_target.database_url")
    def test_inspect_passes_for_reachable_non_pg_lsclusters_listener(
        self, database_url_mock, pg_lsclusters_mock, pg_isready_mock
    ) -> None:
        database_url_mock.return_value = "postgres://pmx@127.0.0.1:55433/pmx_test"
        pg_lsclusters_mock.return_value = []
        pg_isready_mock.return_value = {
            "returncode": 0,
            "stdout": "127.0.0.1:55433 - accepting connections",
            "stderr": "<empty>",
        }
        result = check_local_postgres_target.inspect()
        self.assertEqual(result["status"], "pass")
        self.assertIsNone(result["matched_cluster"])
        self.assertTrue(any("pg_isready reports the endpoint is reachable" in item for item in result["recommendations"]))


if __name__ == "__main__":
    unittest.main()
