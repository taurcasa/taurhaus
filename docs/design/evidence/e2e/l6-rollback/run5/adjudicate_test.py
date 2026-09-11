"""Offline cardinality audit; fixtures are generated in memory."""
import unittest
from adjudicate import assess_delivery

class AccountingContract(unittest.TestCase):
    def test_repeated_reads_are_retained_separately_from_one_submission(self):
        # // Regression: bc1b08f7 checked submission cardinality but omitted the ruling's one-read claim.
        result=assess_delivery({'canonical_submissions':0,'legacy_submissions':1,'explicit_canonical_reads':0,'legacy_history':[{'eventType':'message_read'},{'eventType':'message_read'}],'assistant_replies':[{}]})
        self.assertEqual(result['submissions'],1)
        self.assertEqual(result['reads'],2)
        self.assertFalse(result['single_read'])
        self.assertTrue(result['one_transport_delivery'])

    def test_consumed_only_still_is_a_delivery(self):
        result=assess_delivery({'canonical_submissions':0,'legacy_submissions':0,'explicit_canonical_reads':1,'legacy_history':[],'assistant_replies':[{}]})
        self.assertTrue(result['delivered'])
        self.assertTrue(result['single_read'])
        self.assertFalse(result['one_transport_delivery'])

if __name__=='__main__':unittest.main()
