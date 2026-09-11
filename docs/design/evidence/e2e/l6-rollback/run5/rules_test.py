"""Run4 contract fixtures: generated in memory; no CLI or credential access."""
import copy
import unittest
from rules import downgrade_boundary, same_owner_commit, attached_executor

class Run4Contract(unittest.TestCase):
    def setUp(self):
        self.config={'messaging_format':0,'delivery_owner':'members','delivery_rollback_sha256':'rollback','messaging_downgrade_sha256':'cut'}
        self.authority={'transition':'complete','legacy_cut':'cut'}
        self.projection={'id':'delivery-B','message_id':'B','read':False}
        self.rollback=[{'delivery_id':'delivery-B','recipient':'alpha','stage':'pending'}]

    def test_downgrade_is_the_atomic_ownership_boundary(self):
        # // Regression: 22643c6f inherited 011c2738's format-1 rollback predicate.
        args=[self.config,self.authority,['A','B'],['A','B'],True,True,self.projection,self.rollback,'B']
        downgrade_boundary(*args)
        for index,replacement in [(0,self.config|{'messaging_format':1}), (0,self.config|{'delivery_owner':'team'}), (1,self.authority|{'transition':'prepared'}),(3,['A']), (4,False),(5,False),(6,self.projection|{'read':True}),(7,[])]:
            bad=copy.deepcopy(args);bad[index]=replacement
            with self.subTest(index=index,replacement=replacement),self.assertRaises(AssertionError):downgrade_boundary(*bad)

    def test_same_owner_clears_marker_without_new_epoch_or_request(self):
        # // Regression: 22643c6f required a fresh handoff after downgrade already set members.
        before={'config':self.config,'owner-stopped':{'actor':'lead'},'epoch':{'epoch':3},'handoff':None}
        after=before|{'owner-stopped':None}
        same_owner_commit(before,after,0,True)
        for bad,code,verified in [(before,0,True),(after,1,True),(after,0,False),(after|{'config':self.config|{'delivery_rollback_sha256':'changed'}},0,True)]:
            with self.assertRaises(AssertionError):same_owner_commit(before,bad,code,verified)

    def test_attached_executor_matches_namespace_pid_pane_and_rc(self):
        # // Regression: 22643c6f started a second executor despite self-heal attachment.
        record={'daemon_pid':34,'paneId':'%2'}
        row={'pid':900,'namespace_pid':34,'argv':['/scratch/mesh','daemon','--pane','%2','--name','alpha','--team','T'],'sha256':'rc'}
        self.assertEqual(attached_executor(record,[row],'/scratch/mesh','rc','T'),row)
        self.assertIsNone(attached_executor(record,[],'/scratch/mesh','rc','T'))
        for change in ({'namespace_pid':35},{'sha256':'old'},{'argv':['/scratch/mesh','daemon','--pane','%3','--name','alpha','--team','T']}):
            with self.assertRaises(AssertionError):attached_executor(record,[row|change],'/scratch/mesh','rc','T')
        with self.assertRaises(AssertionError):attached_executor(record,[row,row],'/scratch/mesh','rc','T')

if __name__=='__main__':unittest.main()
