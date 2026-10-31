import unittest

import generate


class HistoricalBuildEnvironmentTest(unittest.TestCase):
    def test_git_transport_appends_without_mutating_inherited_configuration(self):
        inherited = {
            'PATH': 'retained',
            'GIT_CONFIG_COUNT': '2',
            'GIT_CONFIG_KEY_0': 'credential.helper',
            'GIT_CONFIG_VALUE_0': 'retained-helper',
            'GIT_CONFIG_KEY_1': 'http.sslVerify',
            'GIT_CONFIG_VALUE_1': 'true',
        }
        original = inherited.copy()
        child = generate.build_environment(inherited)
        self.assertEqual(inherited, original)
        for key, value in inherited.items():
            if key != 'GIT_CONFIG_COUNT':
                self.assertEqual(child[key], value)
        self.assertEqual(child['GIT_CONFIG_COUNT'], '5')
        self.assertEqual(child['GIT_CONFIG_KEY_2'], f'url.{generate.REPO.as_uri()}.insteadOf')
        self.assertEqual(child['GIT_CONFIG_VALUE_2'], 'https://github.com/vortiq-ai/umya-spreadsheet-serde')
        self.assertEqual(child['GIT_CONFIG_VALUE_3'], 'https://github.com/vortiq-ai/umya-spreadsheet-serde.git')
        self.assertEqual(child['GIT_CONFIG_KEY_4'], 'protocol.file.allow')
        self.assertEqual(child['GIT_CONFIG_VALUE_4'], 'always')

    def test_no_inherited_pairs_starts_at_zero(self):
        child = generate.build_environment({})
        self.assertEqual(child['GIT_CONFIG_COUNT'], '3')
        self.assertEqual(child['CARGO_NET_GIT_FETCH_WITH_CLI'], 'true')
        self.assertEqual(child['CARGO_NET_OFFLINE'], 'true')

    def test_invalid_inherited_counts_are_not_silently_replaced(self):
        for count in ('-1', 'invalid'):
            with self.subTest(count=count), self.assertRaises(ValueError):
                generate.build_environment({'GIT_CONFIG_COUNT': count})


if __name__ == '__main__':
    unittest.main()
